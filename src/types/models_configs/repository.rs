use super::{ModelConfig, ModelConfigViewer};
use uuid::Uuid;

impl ModelConfig {
	pub async fn ensure_system_config(conn: &mut sqlx::PgConnection, model_id: &Uuid, stable_key: &str, name: &str) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, ModelConfig>(
			r#"
			INSERT INTO model_configs (owner_id, model_id, stable_key, name)
			VALUES (NULL, $1, $2, $3)
			ON CONFLICT (model_id) WHERE owner_id IS NULL DO UPDATE
				SET stable_key = EXCLUDED.stable_key,
					name = EXCLUDED.name,
					updated_at = NOW()
			RETURNING
				id,
				owner_id,
				model_id AS model_id,
				stable_key,
				name,
				description,
				icon,
				capabilities,
				input_modalities,
				output_modalities,
				context_length,
				max_output_tokens,
				system_prompt,
				COALESCE(sampling, '{}'::jsonb) AS sampling,
				COALESCE(enabled_tools, '[]'::jsonb) AS enabled_tools,
				COALESCE(is_public, false) AS is_public,
				COALESCE(is_featured, false) AS is_featured,
				COALESCE(is_default, false) AS is_default,
				COALESCE(is_favorite, false) AS is_favorite,
				category,
				COALESCE(tags, '[]'::jsonb) AS tags,
				COALESCE(usage_count, 0) AS usage_count,
				COALESCE(extra_settings, '{}'::jsonb) AS extra_settings,
				created_at,
				updated_at
			"#,
		)
		.bind(model_id)
		.bind(stable_key)
		.bind(name)
		.fetch_one(&mut *conn)
		.await
	}

	pub async fn find_for_user_by_stable_key(pool: &sqlx::PgPool, viewer: ModelConfigViewer<'_>, stable_key: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, ModelConfig>(
			r#"
			SELECT
				id,
				owner_id,
				model_id AS model_id,
				stable_key,
				name,
				description,
				icon,
				capabilities,
				input_modalities,
				output_modalities,
				context_length,
				max_output_tokens,
				system_prompt,
				COALESCE(sampling, '{}'::jsonb) AS sampling,
				COALESCE(enabled_tools, '[]'::jsonb) AS enabled_tools,
				COALESCE(is_public, false) AS is_public,
				COALESCE(is_featured, false) AS is_featured,
				COALESCE(is_default, false) AS is_default,
				COALESCE(is_favorite, false) AS is_favorite,
				category,
				COALESCE(tags, '[]'::jsonb) AS tags,
				COALESCE(usage_count, 0) AS usage_count,
				COALESCE(extra_settings, '{}'::jsonb) AS extra_settings,
				created_at,
				updated_at
			FROM model_configs
			WHERE owner_id = $1 AND stable_key = $2
			"#,
		)
		.bind(viewer.user_id)
		.bind(stable_key)
		.fetch_optional(pool)
		.await
	}

	pub async fn find_system_by_stable_key(pool: &sqlx::PgPool, stable_key: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, ModelConfig>(
			r#"
			SELECT
				id,
				owner_id,
				model_id AS model_id,
				stable_key,
				name,
				description,
				icon,
				capabilities,
				input_modalities,
				output_modalities,
				context_length,
				max_output_tokens,
				system_prompt,
				COALESCE(sampling, '{}'::jsonb) AS sampling,
				COALESCE(enabled_tools, '[]'::jsonb) AS enabled_tools,
				COALESCE(is_public, false) AS is_public,
				COALESCE(is_featured, false) AS is_featured,
				COALESCE(is_default, false) AS is_default,
				COALESCE(is_favorite, false) AS is_favorite,
				category,
				COALESCE(tags, '[]'::jsonb) AS tags,
				COALESCE(usage_count, 0) AS usage_count,
				COALESCE(extra_settings, '{}'::jsonb) AS extra_settings,
				created_at,
				updated_at
			FROM model_configs
			WHERE owner_id IS NULL AND stable_key = $1
			"#,
		)
		.bind(stable_key)
		.fetch_optional(pool)
		.await
	}

	pub(super) async fn find_system_by_model_id_on_connection(conn: &mut sqlx::PgConnection, model_id: &Uuid) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, ModelConfig>(
			r#"
			SELECT
				id,
				owner_id,
				model_id AS model_id,
				stable_key,
				name,
				description,
				icon,
				capabilities,
				input_modalities,
				output_modalities,
				context_length,
				max_output_tokens,
				system_prompt,
				COALESCE(sampling, '{}'::jsonb) AS sampling,
				COALESCE(enabled_tools, '[]'::jsonb) AS enabled_tools,
				COALESCE(is_public, false) AS is_public,
				COALESCE(is_featured, false) AS is_featured,
				COALESCE(is_default, false) AS is_default,
				COALESCE(is_favorite, false) AS is_favorite,
				category,
				COALESCE(tags, '[]'::jsonb) AS tags,
				COALESCE(usage_count, 0) AS usage_count,
				COALESCE(extra_settings, '{}'::jsonb) AS extra_settings,
				created_at,
				updated_at
			FROM model_configs
			WHERE model_id = $1 AND owner_id IS NULL
			"#,
		)
		.bind(model_id)
		.fetch_one(&mut *conn)
		.await
	}
}
