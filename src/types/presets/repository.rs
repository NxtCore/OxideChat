use super::rows::preset_columns;
use super::{Preset, PresetBaseModel, PresetError, PresetResponse, PresetScope, SavePresetRequest};
use crate::types::models::{Model, ModelListPublic};
use crate::types::{AuditEntry, Tool, is_unique_violation};
use crate::logging::AuditLog;
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

fn map_write_error(error: sqlx::Error) -> PresetError {
	if is_unique_violation(&error) { PresetError::SlugTaken } else { PresetError::Database(error) }
}

async fn audit(tx: &mut Transaction<'_, Postgres>, event: &str, actor_id: &Uuid, preset: &Preset) -> Result<(), sqlx::Error> {
	let entry = AuditEntry {
		event,
		actor_id,
		target_type: AUDIT_TARGET,
		target_id: &preset.id,
		metadata: serde_json::json!({ "slug": preset.slug, "scope": preset.scope }),
	};
	AuditLog::write(tx, entry).await
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

	async fn check_references(pool: &PgPool, user_id: &Uuid, request: &SavePresetRequest) -> Result<(), PresetError> {
		let base_usable = Model::find_by_id(pool, &request.base_model_id).await?.is_some_and(|model| model.is_enabled)
			&& Model::can_user_use_model(pool, user_id, &request.base_model_id).await?;
		if !base_usable {
			return Err(PresetError::BaseModelNotAllowed);
		}
		if !request.tool_ids.is_empty() {
			let tools = Tool::find_enabled_for_user(pool, &request.tool_ids, user_id).await?;
			if tools.len() != request.tool_ids.len() {
				return Err(PresetError::ToolsNotAllowed);
			}
		}
		Ok(())
	}

	/// Validates and stores a new personal preset of `user_id`, audited in the same transaction.
	///
	/// # Errors
	/// Returns a [`PresetError`] for invalid fields, a taken slug, an unusable base model or tool, or a database failure.
	pub async fn create_personal(pool: &PgPool, user_id: &Uuid, request: SavePresetRequest) -> Result<Self, PresetError> {
		let request = request.normalized()?;
		Self::check_references(pool, user_id, &request).await?;
		let mut tx = pool.begin().await?;
		let preset = sqlx::query_as::<_, Self>(concat!(
			"INSERT INTO model_presets (scope, owner_id, slug, name, description, icon, base_model_id, system_prompt, sampling, reasoning, \
			 tool_ids, tools_locked, starters, prompt_visible, is_enabled, created_by) \
			 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $2) RETURNING ",
			preset_columns!()
		))
		.bind(PresetScope::Personal.as_str())
		.bind(user_id)
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
		.fetch_one(&mut *tx)
		.await
		.map_err(map_write_error)?;
		audit(&mut tx, EVENT_CREATED, user_id, &preset).await?;
		tx.commit().await?;
		Ok(preset)
	}

	/// Replaces a personal preset owned by `user_id`, audited in the same transaction.
	///
	/// # Errors
	/// Returns [`PresetError::NotFound`] when `user_id` does not own the preset, and the errors of
	/// [`Preset::create_personal`] otherwise.
	pub async fn update_personal(pool: &PgPool, user_id: &Uuid, id: &Uuid, request: SavePresetRequest) -> Result<Self, PresetError> {
		let request = request.normalized()?;
		Self::check_references(pool, user_id, &request).await?;
		let mut tx = pool.begin().await?;
		let preset = sqlx::query_as::<_, Self>(concat!(
			"UPDATE model_presets SET slug = $3, name = $4, description = $5, icon = $6, base_model_id = $7, system_prompt = $8, \
			 sampling = $9, reasoning = $10, tool_ids = $11, tools_locked = $12, starters = $13, prompt_visible = $14, is_enabled = $15, \
			 updated_at = NOW() \
			 WHERE id = $1 AND owner_id = $2 AND scope = 'personal' AND deleted_at IS NULL RETURNING ",
			preset_columns!()
		))
		.bind(id)
		.bind(user_id)
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
		.fetch_optional(&mut *tx)
		.await
		.map_err(map_write_error)?
		.ok_or(PresetError::NotFound)?;
		audit(&mut tx, EVENT_UPDATED, user_id, &preset).await?;
		tx.commit().await?;
		Ok(preset)
	}

	/// Soft-deletes a personal preset owned by `user_id`; old messages keep pointing at it.
	///
	/// # Errors
	/// Returns [`PresetError::NotFound`] when `user_id` does not own the preset.
	pub async fn delete_personal(pool: &PgPool, user_id: &Uuid, id: &Uuid) -> Result<(), PresetError> {
		let mut tx = pool.begin().await?;
		let preset = sqlx::query_as::<_, Self>(concat!(
			"UPDATE model_presets SET deleted_at = NOW(), updated_at = NOW() \
			 WHERE id = $1 AND owner_id = $2 AND scope = 'personal' AND deleted_at IS NULL RETURNING ",
			preset_columns!()
		))
		.bind(id)
		.bind(user_id)
		.fetch_optional(&mut *tx)
		.await?
		.ok_or(PresetError::NotFound)?;
		audit(&mut tx, EVENT_DELETED, user_id, &preset).await?;
		tx.commit().await?;
		Ok(())
	}

	/// Builds the responses for `user_id`, loading every base model in one query.
	///
	/// # Errors
	/// Returns the database error when the base models cannot be loaded.
	pub async fn responses_for_user(pool: &PgPool, user_id: &Uuid, presets: Vec<Self>) -> Result<Vec<PresetResponse>, sqlx::Error> {
		let ids: Vec<Uuid> = presets.iter().filter_map(|preset| preset.base_model_id).collect();
		let bases: HashMap<Uuid, (ModelListPublic, bool)> = Model::list_public_for_user_by_ids(pool, user_id, &ids)
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
				PresetResponse::build(preset, base, user_id)
			})
			.collect())
	}
}
