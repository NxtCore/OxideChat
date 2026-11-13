use super::rows::preset_columns;
use super::{Preset, PresetActor, PresetBaseModel, PresetError, PresetResponse, PresetScope, PresetScopesResponse, PresetTeam, SavePresetRequest};
use crate::logging::AuditLog;
use crate::types::models::{Model, ModelListPublic};
use crate::types::{AssignmentKind, AssignmentRole, AssignmentTarget, AssignmentsResponse, AuditEntry, ReplaceAssignmentsRequest, Tool, is_unique_violation};
use sqlx::postgres::PgArguments;
use sqlx::query::QueryAs;
use sqlx::types::Json;
use sqlx::{PgExecutor, PgPool, Postgres, Transaction};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

const AUDIT_TARGET: &str = "preset";
const EVENT_CREATED: &str = "preset_created";
const EVENT_UPDATED: &str = "preset_updated";
const EVENT_DELETED: &str = "preset_deleted";
const EVENT_SHARED: &str = "preset_sharing_updated";

/// Who a preset is shared with on top of its own scope.
struct Shares {
	everyone: bool,
	rows: Vec<(String, Uuid, String, String)>,
}

impl Shares {
	async fn load(pool: &PgPool, preset_id: &Uuid) -> Result<Self, sqlx::Error> {
		let everyone: Option<bool> = sqlx::query_scalar("SELECT shared_with_everyone FROM model_presets WHERE id = $1").bind(preset_id).fetch_optional(pool).await?;
		let rows = sqlx::query_as::<_, (String, Uuid, String, String)>(
			"SELECT 'team', t.id, t.name::text, s.role::text FROM model_preset_shares s JOIN teams t ON t.id = s.team_id WHERE s.preset_id = $1 \
			 UNION ALL \
			 SELECT 'user', u.id, u.username::text, s.role::text FROM model_preset_shares s JOIN users u ON u.id = s.user_id WHERE s.preset_id = $1 \
			 ORDER BY 1, 3",
		)
		.bind(preset_id)
		.fetch_all(pool)
		.await?;
		Ok(Self {
			everyone: everyone.unwrap_or(false),
			rows,
		})
	}

	/// Whether a share with `id` already gives at least `role`, so keeping it needs no permission.
	fn covers(&self, kind: AssignmentKind, id: &Uuid, role: AssignmentRole) -> bool {
		self.rows
			.iter()
			.any(|(stored, target, _, stored_role)| AssignmentKind::from_stored(stored) == kind && target == id && AssignmentRole::from_stored(stored_role) >= role)
	}

	/// Whether the preset is shared with anyone on top of its own scope.
	async fn exist(executor: impl PgExecutor<'_>, preset_id: &Uuid) -> Result<bool, sqlx::Error> {
		sqlx::query_scalar("SELECT shared_with_everyone OR EXISTS (SELECT 1 FROM model_preset_shares WHERE preset_id = $1) FROM model_presets WHERE id = $1")
			.bind(preset_id)
			.fetch_optional(executor)
			.await
			.map(|shared| shared.unwrap_or(false))
	}
}

macro_rules! visible_to_user {
	() => {
		"deleted_at IS NULL AND (
			(scope = 'personal' AND owner_id = $1)
			OR (scope = 'team' AND team_id IN (SELECT team_id FROM team_members WHERE user_id = $1))
			OR scope = 'installation'
			OR shared_with_everyone
			OR EXISTS (
				SELECT 1 FROM model_preset_shares s WHERE s.preset_id = model_presets.id
				AND (s.user_id = $1 OR s.team_id IN (SELECT team_id FROM team_members WHERE user_id = $1))
			)
		)"
	};
}

pub(super) fn map_write_error(error: sqlx::Error) -> PresetError {
	if is_unique_violation(&error) {
		PresetError::SlugTaken
	} else if error.as_database_error().is_some_and(sqlx::error::DatabaseError::is_foreign_key_violation) {
		PresetError::Invalid
	} else {
		PresetError::Database(error)
	}
}

pub(super) async fn audit(tx: &mut Transaction<'_, Postgres>, event: &str, actor_id: &Uuid, preset: &Preset) -> Result<(), sqlx::Error> {
	let entry = AuditEntry {
		event,
		actor_id,
		target_type: AUDIT_TARGET,
		target_id: &preset.id,
		metadata: serde_json::json!({ "slug": preset.slug, "scope": preset.scope, "team_id": preset.team_id }),
	};
	AuditLog::write(tx, entry).await
}

type PresetQuery<'q> = QueryAs<'q, Postgres, Preset, PgArguments>;

fn bind_fields<'q>(query: PresetQuery<'q>, request: &'q SavePresetRequest) -> PresetQuery<'q> {
	query
		.bind(&request.slug)
		.bind(&request.name)
		.bind(&request.description)
		.bind(&request.icon)
		.bind(request.base_model_id)
		.bind(&request.system_prompt)
		.bind(Json(&request.sampling))
		.bind(Json(&request.reasoning))
		.bind(&request.tool_ids)
		.bind(request.tools_locked)
		.bind(&request.starters)
		.bind(request.prompt_visible)
		.bind(request.is_enabled)
}

impl PresetActor {
	/// Loads the teams `user_id` belongs to and administers and the presets shared with them as an editor; `personal` and `installation_admin` come from the caller's permissions.
	///
	/// # Errors
	/// Returns the database error when the memberships cannot be loaded.
	pub async fn load(pool: &PgPool, user_id: &Uuid, personal: bool, installation_admin: bool) -> Result<Self, sqlx::Error> {
		let memberships = sqlx::query_as::<_, (Uuid, bool)>("SELECT team_id, role = 'admin' FROM team_members WHERE user_id = $1")
			.bind(user_id)
			.fetch_all(pool)
			.await?;
		let editable = sqlx::query_scalar(
			"SELECT DISTINCT preset_id FROM model_preset_shares WHERE role = 'editor' \
			 AND (user_id = $1 OR team_id IN (SELECT team_id FROM team_members WHERE user_id = $1))",
		)
		.bind(user_id)
		.fetch_all(pool)
		.await?;
		Ok(Self {
			user_id: *user_id,
			personal,
			installation_admin,
			admin_teams: memberships.iter().filter(|(_, admin)| *admin).map(|(team, _)| *team).collect(),
			teams: memberships.into_iter().map(|(team, _)| team).collect(),
			editable,
		})
	}

	/// The scopes and teams this actor may create presets in.
	///
	/// # Errors
	/// Returns the database error when the teams cannot be loaded.
	pub async fn scopes(&self, pool: &PgPool) -> Result<PresetScopesResponse, sqlx::Error> {
		let teams = sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM teams WHERE $1 OR id = ANY($2) ORDER BY lower(name)")
			.bind(self.installation_admin)
			.bind(&self.admin_teams)
			.fetch_all(pool)
			.await?;
		Ok(PresetScopesResponse {
			personal: self.personal,
			teams: teams.into_iter().map(|(id, name)| PresetTeam { id, name }).collect(),
			installation: self.installation_admin,
		})
	}
}

impl Preset {
	/// What a preset can be shared with.
	pub const ASSIGNMENT_KINDS: [AssignmentKind; 3] = [AssignmentKind::Everyone, AssignmentKind::Team, AssignmentKind::User];

	/// Presets `user_id` can see: their own, their teams', the installation's and those shared with them, sorted by name.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn list_for_user(pool: &PgPool, user_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!("SELECT ", preset_columns!(), " FROM model_presets WHERE ", visible_to_user!(), " ORDER BY lower(name), slug"))
			.bind(user_id)
			.fetch_all(pool)
			.await
	}

	/// The preset behind a `presets/<slug>` key if `user_id` can see it.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn find_visible_by_slug(pool: &PgPool, user_id: &Uuid, slug: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!("SELECT ", preset_columns!(), " FROM model_presets WHERE slug = $2 AND ", visible_to_user!()))
			.bind(user_id)
			.bind(slug)
			.fetch_optional(pool)
			.await
	}

	/// A preset `user_id` can see, by ID.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn find_visible(pool: &PgPool, user_id: &Uuid, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!("SELECT ", preset_columns!(), " FROM model_presets WHERE id = $2 AND ", visible_to_user!()))
			.bind(user_id)
			.bind(id)
			.fetch_optional(pool)
			.await
	}

	/// A preset the actor can see or manage; installation admins also reach team presets of other teams.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn find_for_actor(pool: &PgPool, actor: &PresetActor, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!(
			"SELECT ",
			preset_columns!(),
			" FROM model_presets WHERE id = $2 AND ((",
			visible_to_user!(),
			") OR ($3 AND deleted_at IS NULL AND scope <> 'personal'))"
		))
		.bind(actor.user_id)
		.bind(id)
		.bind(actor.installation_admin)
		.fetch_optional(pool)
		.await
	}

	/// Whether no other live preset uses `slug`.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn slug_available(pool: &PgPool, slug: &str, exclude_id: Option<&Uuid>) -> Result<bool, sqlx::Error> {
		let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM model_presets WHERE slug = $1 AND deleted_at IS NULL AND ($2::uuid IS NULL OR id <> $2))")
			.bind(slug)
			.bind(exclude_id)
			.fetch_one(pool)
			.await?;
		Ok(!taken)
	}

	async fn check_references(pool: &PgPool, actor: &PresetActor, shared: bool, request: &SavePresetRequest) -> Result<(), PresetError> {
		let base_usable = Model::find_by_id(pool, &request.base_model_id).await?.is_some_and(|model| model.is_enabled)
			&& Model::can_user_use_model(pool, &actor.user_id, &request.base_model_id).await?;
		if !base_usable {
			return Err(PresetError::BaseModelNotAllowed);
		}
		Self::check_tools(pool, actor, shared, &request.tool_ids).await
	}

	async fn check_tools(pool: &PgPool, actor: &PresetActor, shared: bool, tool_ids: &[Uuid]) -> Result<(), PresetError> {
		if !tool_ids.is_empty() {
			let tools = Tool::find_enabled_for_user(pool, tool_ids, &actor.user_id).await?;
			if tools.len() != tool_ids.len() || (shared && tools.iter().any(|tool| tool.owner_id.is_some())) {
				return Err(PresetError::ToolsNotAllowed);
			}
		}
		Ok(())
	}

	async fn find_live(pool: &PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!("SELECT ", preset_columns!(), " FROM model_presets WHERE id = $1 AND deleted_at IS NULL"))
			.bind(id)
			.fetch_optional(pool)
			.await
	}

	async fn find_editable(pool: &PgPool, actor: &PresetActor, id: &Uuid) -> Result<Self, PresetError> {
		Self::find_live(pool, id).await?.filter(|preset| actor.can_edit(preset)).ok_or(PresetError::NotFound)
	}

	async fn find_managed(pool: &PgPool, actor: &PresetActor, id: &Uuid) -> Result<Self, PresetError> {
		Self::find_live(pool, id).await?.filter(|preset| actor.can_manage(preset)).ok_or(PresetError::NotFound)
	}

	/// Validates and stores a new preset in the scope the request names (personal by default), audited in the same transaction.
	///
	/// Shared presets may only bundle system tools.
	///
	/// # Errors
	/// Returns [`PresetError::Forbidden`] when the actor may not manage that scope, and a [`PresetError`] for invalid fields,
	/// a taken slug, an unusable base model or tool, or a database failure.
	pub async fn create(pool: &PgPool, actor: &PresetActor, request: SavePresetRequest) -> Result<Self, PresetError> {
		let request = request.normalized()?;
		let scope = request.scope.unwrap_or(PresetScope::Personal);
		if !actor.can_manage_scope(scope, request.team_id.as_ref()) {
			return Err(PresetError::Forbidden);
		}
		Self::check_references(pool, actor, scope != PresetScope::Personal, &request).await?;
		let owner_id = (scope == PresetScope::Personal).then_some(actor.user_id);
		let mut tx = pool.begin().await?;
		let query = sqlx::query_as::<_, Self>(concat!(
			"INSERT INTO model_presets (scope, owner_id, team_id, created_by, slug, name, description, icon, base_model_id, system_prompt, \
			 sampling, reasoning, tool_ids, tools_locked, starters, prompt_visible, is_enabled) \
			 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17) RETURNING ",
			preset_columns!()
		))
		.bind(scope.as_str())
		.bind(owner_id)
		.bind(request.team_id)
		.bind(actor.user_id);
		let preset = bind_fields(query, &request).fetch_one(&mut *tx).await.map_err(map_write_error)?;
		audit(&mut tx, EVENT_CREATED, &actor.user_id, &preset).await?;
		tx.commit().await?;
		Ok(preset)
	}

	/// Replaces a preset the actor may edit, audited in the same transaction; its scope never changes after it is created.
	///
	/// # Errors
	/// Returns [`PresetError::NotFound`] when the actor may not edit the preset, [`PresetError::Forbidden`] when someone
	/// other than an installation admin turns on a preset an admin turned off, and the errors of [`Preset::create`] otherwise.
	pub async fn update(pool: &PgPool, actor: &PresetActor, id: &Uuid, request: SavePresetRequest) -> Result<Self, PresetError> {
		let current = Self::find_editable(pool, actor, id).await?;
		if current.disabled_by_admin && request.is_enabled && !actor.installation_admin {
			return Err(PresetError::Forbidden);
		}
		let mut request = request;
		request.scope = Some(current.scope());
		request.team_id = current.team_id;
		let request = request.normalized()?;
		let shared = current.scope() != PresetScope::Personal || Shares::exist(pool, id).await?;
		Self::check_references(pool, actor, shared, &request).await?;
		let mut tx = pool.begin().await?;
		let query = sqlx::query_as::<_, Self>(concat!(
			"UPDATE model_presets SET slug = $2, name = $3, description = $4, icon = $5, base_model_id = $6, system_prompt = $7, \
			 sampling = $8, reasoning = $9, tool_ids = $10, tools_locked = $11, starters = $12, prompt_visible = $13, is_enabled = $14, \
			 disabled_by_admin = disabled_by_admin AND NOT $14, updated_at = NOW() \
			 WHERE id = $1 AND deleted_at IS NULL RETURNING ",
			preset_columns!()
		))
		.bind(id);
		let preset = bind_fields(query, &request)
			.fetch_optional(&mut *tx)
			.await
			.map_err(map_write_error)?
			.ok_or(PresetError::NotFound)?;
		if !shared && Shares::exist(&mut *tx, id).await? && Tool::any_personal(&mut *tx, &preset.tool_ids).await? {
			return Err(PresetError::ToolsNotAllowed);
		}
		audit(&mut tx, EVENT_UPDATED, &actor.user_id, &preset).await?;
		tx.commit().await?;
		Ok(preset)
	}

	/// Who can use the preset, in the shared assignment shape: its owner first, then everyone, the teams and the users it is shared with.
	///
	/// `kinds` lists what the actor may share it with, which is nothing unless they manage the preset; teams and users
	/// carry their role, and everyone can only ever use it.
	///
	/// # Errors
	/// Returns the database error when the owner or the shares cannot be loaded.
	pub async fn assignments(&self, pool: &PgPool, actor: &PresetActor) -> Result<AssignmentsResponse, sqlx::Error> {
		let owner = match self.scope() {
			PresetScope::Personal => {
				let label: Option<String> = sqlx::query_scalar("SELECT username::text FROM users WHERE id = $1").bind(self.owner_id).fetch_optional(pool).await?;
				(AssignmentKind::User, self.owner_id, label.unwrap_or_default())
			}
			PresetScope::Team => (AssignmentKind::Team, self.team_id, self.team_name.clone().unwrap_or_default()),
			PresetScope::Installation => (AssignmentKind::Everyone, None, String::new()),
		};
		let shares = Shares::load(pool, &self.id).await?;
		let everyone = shares.everyone.then(|| AssignmentTarget {
			kind: AssignmentKind::Everyone,
			id: None,
			label: String::new(),
			inherited: false,
			owner: false,
			role: None,
		});
		let manages = actor.can_manage(self);
		let mut targets = Vec::with_capacity(shares.rows.len() + 2);
		targets.push(AssignmentTarget {
			kind: owner.0,
			id: owner.1,
			label: owner.2,
			inherited: false,
			owner: true,
			role: None,
		});
		targets.extend(everyone);
		targets.extend(
			shares
				.rows
				.into_iter()
				.map(|(kind, id, label, role)| AssignmentTarget {
					kind: AssignmentKind::from_stored(&kind),
					id: Some(id),
					label,
					inherited: false,
					owner: false,
					role: Some(AssignmentRole::from_stored(&role)),
				})
				.filter(|target| manages || actor.is_share_target(target)),
		);
		let kinds = Self::ASSIGNMENT_KINDS.into_iter().filter(|kind| match kind {
			_ if !manages => false,
			AssignmentKind::Team => actor.installation_admin || !actor.admin_teams.is_empty(),
			kind => actor.can_share_with(*kind, None),
		});
		Ok(AssignmentsResponse {
			kinds: kinds.collect(),
			single: false,
			roles: vec![AssignmentRole::Viewer, AssignmentRole::Editor],
			targets,
		})
	}

	/// Makes the request's targets the only ones a preset the actor manages is shared with, audited in the same transaction.
	///
	/// The owner always keeps the preset, so naming it changes nothing. Shares that stay with the same or a weaker role need no permission; a new one or an upgrade to editor needs
	/// the actor to manage that team, or to be an installation admin for a user or everyone. A team or user with the
	/// editor role may change the preset but neither delete nor share it; everyone can only be given the viewer role.
	///
	/// # Errors
	/// Returns [`PresetError::NotFound`] when the actor may not manage the preset, [`PresetError::Forbidden`] when they may not
	/// share with a new target, [`PresetError::ToolsNotAllowed`] when a shared preset would bundle a personal tool and
	/// [`PresetError::Invalid`] for any other kind of target or an unknown user or team.
	pub async fn replace_assignments(pool: &PgPool, actor: &PresetActor, id: &Uuid, request: &ReplaceAssignmentsRequest) -> Result<Self, PresetError> {
		let current = Self::find_managed(pool, actor, id).await?;
		if !request.is_valid_for(&Self::ASSIGNMENT_KINDS) {
			return Err(PresetError::Invalid);
		}
		let owner_is_everyone = current.scope() == PresetScope::Installation;
		let everyone = !owner_is_everyone && request.targets.iter().any(|target| target.kind == AssignmentKind::Everyone);
		let wants_editor = |kind| request.targets.iter().any(|target| target.kind == kind && target.role == Some(AssignmentRole::Editor));
		if wants_editor(AssignmentKind::Everyone) {
			return Err(PresetError::Invalid);
		}
		let roles = |kind, owner: Option<Uuid>| -> BTreeMap<Uuid, AssignmentRole> {
			let targets = request.targets.iter().filter(|target| target.kind == kind);
			targets.filter_map(|target| Some((target.id?, target.role.unwrap_or(AssignmentRole::Viewer)))).filter(|(id, _)| owner != Some(*id)).collect()
		};
		let teams = roles(AssignmentKind::Team, current.team_id);
		let users = roles(AssignmentKind::User, current.owner_id);
		let team_ids: Vec<Uuid> = teams.keys().copied().collect();
		let user_ids: Vec<Uuid> = users.keys().copied().collect();
		let stored = |roles: &BTreeMap<Uuid, AssignmentRole>| -> Vec<&'static str> { roles.values().map(|role| role.as_str()).collect() };

		let shares = Shares::load(pool, id).await?;
		let mut new_teams = teams.iter().filter(|(team, role)| !shares.covers(AssignmentKind::Team, team, **role)).map(|(team, _)| team);
		let new_user = users.iter().any(|(user, role)| !shares.covers(AssignmentKind::User, user, *role));
		let allowed = new_teams.all(|team| actor.can_share_with(AssignmentKind::Team, Some(team)))
			&& (!new_user || actor.can_share_with(AssignmentKind::User, None))
			&& (!everyone || shares.everyone || actor.can_share_with(AssignmentKind::Everyone, None));
		if !allowed {
			return Err(PresetError::Forbidden);
		}
		let shared = current.scope() != PresetScope::Personal || everyone || !team_ids.is_empty() || !user_ids.is_empty();

		let mut tx = pool.begin().await?;
		let preset = sqlx::query_as::<_, Self>(concat!(
			"UPDATE model_presets SET shared_with_everyone = $2, updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL RETURNING ",
			preset_columns!()
		))
		.bind(id)
		.bind(everyone)
		.fetch_optional(&mut *tx)
		.await?
		.ok_or(PresetError::NotFound)?;
		if shared && Tool::any_personal(&mut *tx, &preset.tool_ids).await? {
			return Err(PresetError::ToolsNotAllowed);
		}
		sqlx::query("DELETE FROM model_preset_shares WHERE preset_id = $1").bind(id).execute(&mut *tx).await?;
		sqlx::query(
			"INSERT INTO model_preset_shares (preset_id, team_id, user_id, role) \
			 SELECT $1, team.id, NULL, team.role FROM unnest($2::uuid[], $3::text[]) AS team(id, role) \
			 UNION ALL SELECT $1, NULL, member.id, member.role FROM unnest($4::uuid[], $5::text[]) AS member(id, role)",
		)
		.bind(id)
		.bind(&team_ids)
		.bind(stored(&teams))
		.bind(&user_ids)
		.bind(stored(&users))
		.execute(&mut *tx)
		.await
		.map_err(map_write_error)?;
		let editors: Vec<&Uuid> = teams.iter().chain(&users).filter(|(_, role)| **role == AssignmentRole::Editor).map(|(id, _)| id).collect();
		let entry = AuditEntry {
			event: EVENT_SHARED,
			actor_id: &actor.user_id,
			target_type: AUDIT_TARGET,
			target_id: id,
			metadata: serde_json::json!({ "slug": preset.slug, "everyone": everyone, "team_ids": team_ids, "user_ids": user_ids, "editor_ids": editors }),
		};
		AuditLog::write(&mut tx, entry).await?;
		tx.commit().await?;
		Ok(preset)
	}

	/// Soft-deletes a preset the actor manages; old messages keep pointing at it.
	///
	/// # Errors
	/// Returns [`PresetError::NotFound`] when the actor may not manage the preset.
	pub async fn delete(pool: &PgPool, actor: &PresetActor, id: &Uuid) -> Result<(), PresetError> {
		Self::find_managed(pool, actor, id).await?;
		let mut tx = pool.begin().await?;
		let preset = sqlx::query_as::<_, Self>(concat!(
			"UPDATE model_presets SET deleted_at = NOW(), updated_at = NOW() WHERE id = $1 AND deleted_at IS NULL RETURNING ",
			preset_columns!()
		))
		.bind(id)
		.fetch_optional(&mut *tx)
		.await?
		.ok_or(PresetError::NotFound)?;
		audit(&mut tx, EVENT_DELETED, &actor.user_id, &preset).await?;
		tx.commit().await?;
		Ok(())
	}

	/// Builds the responses for the actor, loading every base model in one query.
	///
	/// # Errors
	/// Returns the database error when the base models cannot be loaded.
	pub async fn responses_for_user(pool: &PgPool, actor: &PresetActor, presets: Vec<Self>) -> Result<Vec<PresetResponse>, sqlx::Error> {
		let ids: Vec<Uuid> = presets.iter().filter_map(|preset| preset.base_model_id).collect();
		let bases: HashMap<Uuid, (ModelListPublic, bool)> = Model::list_public_for_user_by_ids(pool, &actor.user_id, &ids)
			.await?
			.into_iter()
			.map(|(model, allowed)| (model.id, (model, allowed)))
			.collect();
		let preset_ids: Vec<Uuid> = presets.iter().map(|preset| preset.id).collect();
		let used: HashSet<Uuid> =
			sqlx::query_scalar("SELECT id FROM unnest($1::uuid[]) AS id WHERE EXISTS (SELECT 1 FROM gateway_request_logs l WHERE l.preset_id = id)")
				.bind(&preset_ids)
				.fetch_all(pool)
				.await?
				.into_iter()
				.collect();
		Ok(presets
			.into_iter()
			.map(|preset| {
				let base = preset
					.base_model_id
					.and_then(|id| bases.get(&id))
					.map(|(model, allowed)| PresetBaseModel { model: model.clone(), allowed: *allowed });
				let has_gateway_usage = used.contains(&preset.id);
				PresetResponse {
					has_gateway_usage,
					..PresetResponse::build(preset, base, actor)
				}
			})
			.collect())
	}
}
