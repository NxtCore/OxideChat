use super::repository::{audit, map_write_error};
use super::rows::preset_columns;
use super::{AdminPresetQuery, AdminPresetResponse, AdminUpdatePresetRequest, PRESET_KEY_PREFIX, Preset, PresetError, PresetOwner, PresetScope, PresetTeam, is_valid_preset_slug};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

const EVENT_ADMIN_UPDATED: &str = "preset_admin_updated";

#[derive(sqlx::FromRow)]
struct AdminPresetRow {
	id: Uuid,
	slug: String,
	scope: String,
	name: String,
	icon: Option<String>,
	team_id: Option<Uuid>,
	team_name: Option<String>,
	owner_id: Option<Uuid>,
	owner_username: Option<String>,
	base_model_name: Option<String>,
	is_enabled: bool,
	updated_at: DateTime<Utc>,
}

impl From<AdminPresetRow> for AdminPresetResponse {
	fn from(row: AdminPresetRow) -> Self {
		Self {
			id: row.id,
			key: format!("{PRESET_KEY_PREFIX}{}", row.slug),
			slug: row.slug,
			scope: PresetScope::from_stored(&row.scope),
			name: row.name,
			icon: row.icon,
			team: row.team_id.zip(row.team_name).map(|(id, name)| PresetTeam { id, name }),
			owner: row.owner_id.zip(row.owner_username).map(|(id, username)| PresetOwner { id, username }),
			base_model_name: row.base_model_name,
			is_enabled: row.is_enabled,
			updated_at: row.updated_at,
		}
	}
}

impl Preset {
	/// Every live preset of the installation, optionally filtered by scope and team, sorted by name.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn admin_list(pool: &PgPool, query: &AdminPresetQuery) -> Result<Vec<AdminPresetResponse>, sqlx::Error> {
		Self::admin_rows(pool, query, None).await
	}

	async fn admin_rows(pool: &PgPool, query: &AdminPresetQuery, id: Option<&Uuid>) -> Result<Vec<AdminPresetResponse>, sqlx::Error> {
		let rows = sqlx::query_as::<_, AdminPresetRow>(
			"SELECT p.id, p.slug, p.scope, p.name, p.icon, p.team_id, t.name AS team_name, p.owner_id, u.username AS owner_username, \
			        m.display_name AS base_model_name, p.is_enabled, p.updated_at \
			 FROM model_presets p \
			 LEFT JOIN teams t ON t.id = p.team_id \
			 LEFT JOIN users u ON u.id = p.owner_id \
			 LEFT JOIN models m ON m.id = p.base_model_id \
			 WHERE p.deleted_at IS NULL AND ($1::text IS NULL OR p.scope = $1) AND ($2::uuid IS NULL OR p.team_id = $2) AND ($3::uuid IS NULL OR p.id = $3) \
			 ORDER BY lower(p.name), p.slug",
		)
		.bind(query.scope.map(PresetScope::as_str))
		.bind(query.team_id)
		.bind(id)
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(AdminPresetResponse::from).collect())
	}

	/// Renames or turns on or off any preset, audited in the same transaction.
	///
	/// # Errors
	/// Returns [`PresetError::InvalidSlug`] or [`PresetError::SlugTaken`] for a bad slug and [`PresetError::NotFound`]
	/// when the preset does not exist.
	pub async fn admin_update(pool: &PgPool, actor_id: &Uuid, id: &Uuid, request: AdminUpdatePresetRequest) -> Result<AdminPresetResponse, PresetError> {
		let slug = request.slug.map(|slug| slug.trim().to_string());
		if slug.as_deref().is_some_and(|slug| !is_valid_preset_slug(slug)) {
			return Err(PresetError::InvalidSlug);
		}
		let mut tx = pool.begin().await?;
		let preset = sqlx::query_as::<_, Self>(concat!(
			"UPDATE model_presets SET slug = COALESCE($2, slug), is_enabled = COALESCE($3, is_enabled), updated_at = NOW() \
			 WHERE id = $1 AND deleted_at IS NULL RETURNING ",
			preset_columns!()
		))
		.bind(id)
		.bind(slug)
		.bind(request.is_enabled)
		.fetch_optional(&mut *tx)
		.await
		.map_err(map_write_error)?
		.ok_or(PresetError::NotFound)?;
		audit(&mut tx, EVENT_ADMIN_UPDATED, actor_id, &preset).await?;
		tx.commit().await?;
		Self::admin_rows(pool, &AdminPresetQuery::default(), Some(&preset.id)).await?.pop().ok_or(PresetError::NotFound)
	}
}
