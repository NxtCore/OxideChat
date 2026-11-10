use super::rows::preset_columns;
use super::{Preset, PresetActor, PresetBaseModel, PresetError, PresetResponse, PresetScope, PresetScopesResponse, PresetTeam, SavePresetRequest};
use crate::logging::AuditLog;
use crate::types::models::{Model, ModelListPublic};
use crate::types::{AuditEntry, Tool, is_unique_violation};
use sqlx::postgres::PgArguments;
use sqlx::query::QueryAs;
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

const AUDIT_TARGET: &str = "preset";
const EVENT_CREATED: &str = "preset_created";
const EVENT_UPDATED: &str = "preset_updated";
const EVENT_DELETED: &str = "preset_deleted";

macro_rules! visible_to_user {
	() => {
		"deleted_at IS NULL AND (
			(scope = 'personal' AND owner_id = $1)
			OR (scope = 'team' AND team_id IN (SELECT team_id FROM team_members WHERE user_id = $1))
			OR scope = 'installation'
		)"
	};
}

pub(super) fn map_write_error(error: sqlx::Error) -> PresetError {
	if is_unique_violation(&error) { PresetError::SlugTaken } else { PresetError::Database(error) }
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
	/// Loads the teams `user_id` administers; `personal` and `installation_admin` come from the caller's permissions.
	///
	/// # Errors
	/// Returns the database error when the memberships cannot be loaded.
	pub async fn load(pool: &PgPool, user_id: &Uuid, personal: bool, installation_admin: bool) -> Result<Self, sqlx::Error> {
		let admin_teams = sqlx::query_scalar("SELECT team_id FROM team_members WHERE user_id = $1 AND role = 'admin'")
			.bind(user_id)
			.fetch_all(pool)
			.await?;
		Ok(Self {
			user_id: *user_id,
			personal,
			installation_admin,
			admin_teams,
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
	/// Presets `user_id` can see: their own, their teams' and the installation's, sorted by name.
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

	async fn check_references(pool: &PgPool, actor: &PresetActor, scope: PresetScope, request: &SavePresetRequest) -> Result<(), PresetError> {
		let base_usable = Model::find_by_id(pool, &request.base_model_id).await?.is_some_and(|model| model.is_enabled)
			&& Model::can_user_use_model(pool, &actor.user_id, &request.base_model_id).await?;
		if !base_usable {
			return Err(PresetError::BaseModelNotAllowed);
		}
		if !request.tool_ids.is_empty() {
			let tools = Tool::find_enabled_for_user(pool, &request.tool_ids, &actor.user_id).await?;
			let shared = scope != PresetScope::Personal;
			if tools.len() != request.tool_ids.len() || (shared && tools.iter().any(|tool| tool.owner_id.is_some())) {
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
		Self::check_references(pool, actor, scope, &request).await?;
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

	/// Replaces a preset the actor manages, audited in the same transaction; its scope never changes.
	///
	/// # Errors
	/// Returns [`PresetError::NotFound`] when the actor may not manage the preset, and the errors of [`Preset::create`] otherwise.
	pub async fn update(pool: &PgPool, actor: &PresetActor, id: &Uuid, request: SavePresetRequest) -> Result<Self, PresetError> {
		let current = Self::find_managed(pool, actor, id).await?;
		let mut request = request;
		request.scope = Some(current.scope());
		request.team_id = current.team_id;
		let request = request.normalized()?;
		Self::check_references(pool, actor, current.scope(), &request).await?;
		let mut tx = pool.begin().await?;
		let query = sqlx::query_as::<_, Self>(concat!(
			"UPDATE model_presets SET slug = $2, name = $3, description = $4, icon = $5, base_model_id = $6, system_prompt = $7, \
			 sampling = $8, reasoning = $9, tool_ids = $10, tools_locked = $11, starters = $12, prompt_visible = $13, is_enabled = $14, \
			 updated_at = NOW() \
			 WHERE id = $1 AND deleted_at IS NULL RETURNING ",
			preset_columns!()
		))
		.bind(id);
		let preset = bind_fields(query, &request)
			.fetch_optional(&mut *tx)
			.await
			.map_err(map_write_error)?
			.ok_or(PresetError::NotFound)?;
		audit(&mut tx, EVENT_UPDATED, &actor.user_id, &preset).await?;
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
		Ok(presets
			.into_iter()
			.map(|preset| {
				let base = preset
					.base_model_id
					.and_then(|id| bases.get(&id))
					.map(|(model, allowed)| PresetBaseModel { model: model.clone(), allowed: *allowed });
				PresetResponse::build(preset, base, actor)
			})
			.collect())
	}
}
