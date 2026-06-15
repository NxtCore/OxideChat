use super::Model;
use sqlx::types::Json;
use uuid::Uuid;

impl Model {
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
