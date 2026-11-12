use super::Model;
use crate::logging::AuditLog;
use crate::types::{AUDIT_TEAM_TARGET, AssignmentKind, AssignmentTarget, AssignmentsResponse, AuditEntry, EVENT_TEAM_MODELS_UPDATED};
use sqlx::types::Json;
use uuid::Uuid;

impl Model {
	/// Model access is granted to teams; grants to roles or users are not enforced anywhere.
	pub const ASSIGNMENT_KINDS: [AssignmentKind; 1] = [AssignmentKind::Team];

	/// The teams that may use a model; teams that get it through "all models" or a provider grant are `inherited`.
	///
	/// # Errors
	///
	/// Returns the database error when the query fails.
	pub async fn assignments(pool: &sqlx::PgPool, model_id: &Uuid) -> Result<AssignmentsResponse, sqlx::Error> {
		let rows = sqlx::query_as::<_, (String, Uuid, String, bool)>(
			r#"
			SELECT 'team', t.id, t.name::text,
			       NOT EXISTS (SELECT 1 FROM team_model_access a WHERE a.team_id = t.id AND a.model_id = m.id)
			FROM teams t
			JOIN models m ON m.id = $1
			WHERE t.allow_all_models
			   OR EXISTS (SELECT 1 FROM team_model_access a WHERE a.team_id = t.id AND (a.model_id = m.id OR a.provider_id = m.provider_id))
			ORDER BY lower(t.name)
			"#,
		)
		.bind(model_id)
		.fetch_all(pool)
		.await?;
		Ok(AssignmentsResponse {
			kinds: Self::ASSIGNMENT_KINDS.to_vec(),
			single: false,
			targets: rows.into_iter().map(AssignmentTarget::from).collect(),
		})
	}

	/// Makes `team_ids` the only teams with their own grant for a model, auditing every team that changed in one transaction.
	///
	/// # Errors
	///
	/// Returns an error when the model or a team does not exist or a change cannot be stored or audited; nothing is changed then.
	pub async fn replace_assignments(pool: &sqlx::PgPool, actor_id: &Uuid, model_id: &Uuid, team_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		let removed: Vec<Uuid> = sqlx::query_scalar("DELETE FROM team_model_access WHERE model_id = $1 AND team_id <> ALL($2) RETURNING team_id")
			.bind(model_id)
			.bind(team_ids)
			.fetch_all(&mut *tx)
			.await?;
		let added: Vec<Uuid> = sqlx::query_scalar(
			"INSERT INTO team_model_access (team_id, model_id) SELECT team_id, $1 FROM unnest($2::uuid[]) AS team_id ON CONFLICT DO NOTHING RETURNING team_id",
		)
		.bind(model_id)
		.bind(team_ids)
		.fetch_all(&mut *tx)
		.await?;
		let changes = removed.iter().map(|team_id| (team_id, false)).chain(added.iter().map(|team_id| (team_id, true)));
		for (team_id, granted) in changes {
			let entry = AuditEntry {
				event: EVENT_TEAM_MODELS_UPDATED,
				actor_id,
				target_type: AUDIT_TEAM_TARGET,
				target_id: team_id,
				metadata: serde_json::json!({ "model_id": model_id, "granted": granted }),
			};
			AuditLog::write(&mut tx, entry).await?;
		}
		tx.commit().await
	}

	pub async fn patch(conn: &mut sqlx::PgConnection, id: &Uuid, display_name: Option<&str>, is_enabled: Option<bool>) -> Result<Option<Self>, sqlx::Error> {
		if display_name.is_none() && is_enabled.is_none() {
			return Self::find_by_id_on_connection(conn, id).await;
		}

		sqlx::query_as!(
			Model,
			r#"
			UPDATE models
			SET
				display_name = COALESCE($2, display_name),
				is_enabled = COALESCE($3, is_enabled),
				updated_at = NOW()
			WHERE id = $1
			RETURNING
				id,
				provider_id,
				model_id,
				display_name,
				COALESCE(capabilities, '[]'::jsonb) AS "capabilities!: Json<Vec<String>>",
				COALESCE(input_modalities, '[]'::jsonb) AS "input_modalities!: Json<Vec<String>>",
				COALESCE(output_modalities, '[]'::jsonb) AS "output_modalities!: Json<Vec<String>>",
				context_length,
				max_tokens,
				COALESCE(is_enabled, false) AS "is_enabled!",
				created_at,
				updated_at
			"#,
			id,
			display_name,
			is_enabled,
		)
		.fetch_optional(&mut *conn)
		.await
	}

	async fn find_by_id_on_connection(conn: &mut sqlx::PgConnection, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as!(
			Model,
			r#"
			SELECT
				id,
				provider_id,
				model_id,
				display_name,
				COALESCE(capabilities, '[]'::jsonb) AS "capabilities!: Json<Vec<String>>",
				COALESCE(input_modalities, '[]'::jsonb) AS "input_modalities!: Json<Vec<String>>",
				COALESCE(output_modalities, '[]'::jsonb) AS "output_modalities!: Json<Vec<String>>",
				context_length,
				max_tokens,
				COALESCE(is_enabled, false) AS "is_enabled!",
				created_at,
				updated_at
			FROM models
			WHERE id = $1
			"#,
			id,
		)
		.fetch_optional(&mut *conn)
		.await
	}
}
