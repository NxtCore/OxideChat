//! Per-record access: records list labels, and each label is held by teams, roles or users of this installation.

use super::{AccessLabel, AccessLabelsResponse, KnowledgeActor, KnowledgeBase, RagConnection, RagError, SearchAccess, VectorStore};
use crate::logging::AuditLog;
use crate::types::{AssignmentKind, AssignmentTarget, AssignmentsResponse, AuditEntry, JobState, ReplaceAssignmentsRequest};
use sqlx::PgPool;
use std::collections::BTreeMap;
use uuid::Uuid;

/// Longest access label.
pub const MAX_LABEL_CHARS: usize = 200;

/// Checks a label typed by an admin or taken from a path.
///
/// # Errors
///
/// Returns `InvalidLabel` when it is empty or longer than [`MAX_LABEL_CHARS`].
pub fn check_label(label: &str) -> Result<&str, RagError> {
	if label.is_empty() || label.chars().count() > MAX_LABEL_CHARS {
		Err(RagError::InvalidLabel)
	} else {
		Ok(label)
	}
}

impl KnowledgeBase {
	/// Who may hold a label.
	pub const LABEL_KINDS: [AssignmentKind; 3] = [AssignmentKind::Team, AssignmentKind::Role, AssignmentKind::User];

	/// The labels `user_id` holds in this knowledge base, loaded live so a lost team or role applies to the next search.
	///
	/// # Errors
	///
	/// Returns a database error when the query fails.
	pub async fn held_labels(&self, pool: &PgPool, user_id: &Uuid) -> Result<Vec<String>, sqlx::Error> {
		if self.view.access.is_none() {
			return Ok(Vec::new());
		}
		sqlx::query_scalar(
			"SELECT DISTINCT label FROM knowledge_base_access_labels WHERE knowledge_base_id = $1 AND (
				user_id = $2
				OR team_id IN (SELECT team_id FROM team_members WHERE user_id = $2)
				OR role_id IN (SELECT role_id FROM user_roles WHERE user_id = $2)
			 ) ORDER BY label",
		)
		.bind(self.id)
		.bind(user_id)
		.fetch_all(pool)
		.await
	}

	/// Mapped labels and how many teams, roles and users hold each.
	async fn mapped_labels(&self, pool: &PgPool) -> Result<BTreeMap<String, i64>, sqlx::Error> {
		let rows: Vec<(String, i64)> = sqlx::query_as("SELECT label, count(*) FROM knowledge_base_access_labels WHERE knowledge_base_id = $1 GROUP BY label")
			.bind(self.id)
			.fetch_all(pool)
			.await?;
		Ok(rows.into_iter().collect())
	}

	/// The labels found in the collection and the mapped ones, each with how many hold it.
	///
	/// A store that cannot be read is reported in the response; the mapped labels are still listed.
	///
	/// # Errors
	///
	/// Returns a database error.
	pub async fn access_labels(&self, state: &JobState) -> Result<AccessLabelsResponse, RagError> {
		let mapped = self.mapped_labels(&state.db).await?;
		let (found, error) = if self.view.access.is_some() {
			match self.discover_labels(state).await? {
				Ok(found) => (found, None),
				Err(error) => (Vec::new(), Some(error)),
			}
		} else {
			(Vec::new(), None)
		};
		let mut labels: BTreeMap<&str, AccessLabel> = BTreeMap::new();
		for label in &found {
			labels.insert(label, AccessLabel {
				label: label.clone(),
				found: true,
				holders: 0,
			});
		}
		for (label, holders) in &mapped {
			labels
				.entry(label)
				.or_insert_with(|| AccessLabel {
					label: label.clone(),
					found: false,
					holders: 0,
				})
				.holders = u32::try_from(*holders).unwrap_or(u32::MAX);
		}
		let unassigned = labels.values().filter(|label| label.found && label.holders == 0).count();
		Ok(AccessLabelsResponse {
			field: self.view.access.clone(),
			labels: labels.into_values().collect(),
			unassigned: u32::try_from(unassigned).unwrap_or(u32::MAX),
			error,
		})
	}

	/// The labels of the collection, or why the store could not be read.
	async fn discover_labels(&self, state: &JobState) -> Result<Result<Vec<String>, super::StoreError>, RagError> {
		let connection = RagConnection::find(&state.db, &self.connection_id).await?.ok_or(RagError::NotFound)?;
		Ok(match VectorStore::connect(&connection, &state.db, None).await {
			Ok(store) => store.access_labels(&self.view).await,
			Err(error) => Err(error),
		})
	}

	/// Who holds `label`, in the shared assignment shape; only knowledge admins may change it.
	///
	/// # Errors
	///
	/// Returns a database error when the holders cannot be loaded.
	pub async fn label_assignments(&self, pool: &PgPool, actor: &KnowledgeActor, label: &str) -> Result<AssignmentsResponse, sqlx::Error> {
		let rows = sqlx::query_as::<_, (String, Uuid, String)>(
			"SELECT 'team', t.id, t.name::text FROM knowledge_base_access_labels a JOIN teams t ON t.id = a.team_id WHERE a.knowledge_base_id = $1 AND a.label = $2
			 UNION ALL SELECT 'role', r.id, r.name::text FROM knowledge_base_access_labels a JOIN roles r ON r.id = a.role_id WHERE a.knowledge_base_id = $1 AND a.label = $2
			 UNION ALL SELECT 'user', u.id, u.username::text FROM knowledge_base_access_labels a JOIN users u ON u.id = a.user_id WHERE a.knowledge_base_id = $1 AND a.label = $2
			 ORDER BY 1, 3",
		)
		.bind(self.id)
		.bind(label)
		.fetch_all(pool)
		.await?;
		Ok(AssignmentsResponse {
			kinds: if actor.manage { Self::LABEL_KINDS.to_vec() } else { Vec::new() },
			single: false,
			roles: Vec::new(),
			targets: rows
				.into_iter()
				.map(|(kind, id, name)| AssignmentTarget {
					kind: AssignmentKind::from_stored(&kind),
					id: Some(id),
					label: name,
					inherited: false,
					owner: false,
					role: None,
				})
				.collect(),
		})
	}

	/// Makes the request's targets the only holders of `label`, audited in the same transaction.
	///
	/// # Errors
	///
	/// Returns `Forbidden` unless the actor manages knowledge, `InvalidLabel`, `InvalidAssignment` for another kind or an unknown target, or a database error.
	pub async fn replace_label_assignments(&self, pool: &PgPool, actor: &KnowledgeActor, label: &str, request: &ReplaceAssignmentsRequest) -> Result<(), RagError> {
		if !actor.manage {
			return Err(RagError::Forbidden);
		}
		let label = check_label(label)?;
		if !request.is_valid_for(&Self::LABEL_KINDS) {
			return Err(RagError::InvalidAssignment);
		}
		let users = request.ids(AssignmentKind::User);
		let teams = request.ids(AssignmentKind::Team);
		let roles = request.ids(AssignmentKind::Role);
		let mut tx = pool.begin().await?;
		sqlx::query("DELETE FROM knowledge_base_access_labels WHERE knowledge_base_id = $1 AND label = $2")
			.bind(self.id)
			.bind(label)
			.execute(&mut *tx)
			.await?;
		sqlx::query(
			"INSERT INTO knowledge_base_access_labels (knowledge_base_id, label, user_id, team_id, role_id)
			 SELECT $1, $2, id, NULL::uuid, NULL::uuid FROM unnest($3::uuid[]) AS id
			 UNION ALL SELECT $1, $2, NULL, id, NULL FROM unnest($4::uuid[]) AS id
			 UNION ALL SELECT $1, $2, NULL, NULL, id FROM unnest($5::uuid[]) AS id",
		)
		.bind(self.id)
		.bind(label)
		.bind(&users)
		.bind(&teams)
		.bind(&roles)
		.execute(&mut *tx)
		.await
		.map_err(|error| RagError::foreign_key_or(error, RagError::InvalidAssignment))?;
		let metadata = serde_json::json!({ "name": self.name, "label": label, "users": users, "teams": teams, "roles": roles });
		AuditLog::write(
			&mut tx,
			AuditEntry {
				event: "knowledge_base_access_label_updated",
				actor_id: &actor.user_id,
				target_type: "knowledge_base",
				target_id: &self.id,
				metadata,
			},
		)
		.await?;
		tx.commit().await?;
		Ok(())
	}

	/// What `user_id` may find here: whether the base is shared with them and the labels they hold.
	///
	/// # Errors
	///
	/// Returns a database error.
	pub async fn access_for(&self, state: &JobState, user_id: &Uuid) -> Result<(SearchAccess, Vec<String>), RagError> {
		let usable = !Self::accessible(&state.db, user_id, &[self.id]).await?.is_empty();
		let held = self.held_labels(&state.db, user_id).await?;
		let mut access = SearchAccess {
			usable,
			per_record: self.view.access.is_some(),
			held: u32::try_from(held.len()).unwrap_or(u32::MAX),
			labels: 0,
			unassigned: None,
		};
		if access.per_record {
			let labels = self.access_labels(state).await?;
			access.labels = u32::try_from(labels.labels.len()).unwrap_or(u32::MAX);
			access.unassigned = labels.error.is_none().then_some(labels.unassigned);
		}
		Ok((access, held))
	}

	/// Writes the `rag_search_as` audit row before an admin searches as another user.
	///
	/// # Errors
	///
	/// Returns a database error.
	pub async fn audit_search_as(&self, pool: &PgPool, actor_id: &Uuid, user_id: &Uuid, query: &str) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		AuditLog::write(
			&mut tx,
			AuditEntry {
				event: "rag_search_as",
				actor_id,
				target_type: "knowledge_base",
				target_id: &self.id,
				metadata: serde_json::json!({ "name": self.name, "user_id": user_id, "query": query }),
			},
		)
		.await?;
		tx.commit().await
	}
}
