use super::Preset;
use super::rows::preset_columns;
use crate::types::GatewayAuthContext;
use sqlx::PgPool;

macro_rules! visible_to_key {
	() => {
		"deleted_at IS NULL AND is_enabled AND base_model_id IS NOT NULL AND (
			(scope = 'personal' AND owner_id = $1)
			OR (scope = 'team' AND team_id = $2)
			OR scope = 'installation'
		)"
	};
}

impl Preset {
	/// Enabled presets an API key can call: its owner's personal presets, its pinned team's and the installation's.
	///
	/// Service-account keys have no owner and never see personal presets. Base-model access is checked by the caller.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn list_for_gateway(pool: &PgPool, context: &GatewayAuthContext) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!(
			"SELECT ",
			preset_columns!(),
			" FROM model_presets WHERE ",
			visible_to_key!(),
			" ORDER BY slug"
		))
		.bind(context.user_id)
		.bind(context.team_id)
		.fetch_all(pool)
		.await
	}

	/// The enabled preset behind `presets/<slug>` if the API key can call it.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn find_for_gateway(pool: &PgPool, context: &GatewayAuthContext, slug: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!("SELECT ", preset_columns!(), " FROM model_presets WHERE slug = $3 AND ", visible_to_key!()))
			.bind(context.user_id)
			.bind(context.team_id)
			.bind(slug)
			.fetch_optional(pool)
			.await
	}
}
