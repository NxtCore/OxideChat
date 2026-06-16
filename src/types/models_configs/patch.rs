use super::ModelConfig;
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder, types::Json};
use uuid::Uuid;

enum ConfigValue<'a> {
	Text(Option<&'a str>),
	Int(Option<i32>),
	Bool(bool),
	Json(Option<&'a Value>),
	JsonMerge(&'a Value),
}

pub enum ModelConfigPatchField<'a> {
	Description(Option<&'a str>),
	Icon(Option<&'a str>),
	SystemPrompt(Option<&'a str>),
	SamplingMerge(&'a Value),
	InputModalities(Option<&'a Value>),
	OutputModalities(Option<&'a Value>),
	ContextLength(Option<i32>),
	MaxOutputTokens(Option<i32>),
	EnabledTools(Option<&'a Value>),
	IsPublic(bool),
	IsFeatured(bool),
	IsDefault(bool),
	IsFavorite(bool),
	Category(Option<&'a str>),
	Tags(Option<&'a Value>),
	ExtraSettings(Option<&'a Value>),
}

impl<'a> ModelConfigPatchField<'a> {
	fn column_and_value(&self) -> (&'static str, ConfigValue<'a>) {
		match self {
			Self::Description(v) => ("description", ConfigValue::Text(*v)),
			Self::Icon(v) => ("icon", ConfigValue::Text(*v)),
			Self::SystemPrompt(v) => ("system_prompt", ConfigValue::Text(*v)),
			Self::SamplingMerge(v) => ("sampling", ConfigValue::JsonMerge(*v)),
			Self::InputModalities(v) => ("input_modalities", ConfigValue::Json(*v)),
			Self::OutputModalities(v) => ("output_modalities", ConfigValue::Json(*v)),
			Self::ContextLength(v) => ("context_length", ConfigValue::Int(*v)),
			Self::MaxOutputTokens(v) => ("max_output_tokens", ConfigValue::Int(*v)),
			Self::EnabledTools(v) => ("enabled_tools", ConfigValue::Json(*v)),
			Self::IsPublic(v) => ("is_public", ConfigValue::Bool(*v)),
			Self::IsFeatured(v) => ("is_featured", ConfigValue::Bool(*v)),
			Self::IsDefault(v) => ("is_default", ConfigValue::Bool(*v)),
			Self::IsFavorite(v) => ("is_favorite", ConfigValue::Bool(*v)),
			Self::Category(v) => ("category", ConfigValue::Text(*v)),
			Self::Tags(v) => ("tags", ConfigValue::Json(*v)),
			Self::ExtraSettings(v) => ("extra_settings", ConfigValue::Json(*v)),
		}
	}
}

impl ModelConfig {
	pub async fn patch_system_config(conn: &mut sqlx::PgConnection, model_id: &Uuid, fields: &[ModelConfigPatchField<'_>]) -> Result<Self, sqlx::Error> {
		if fields.is_empty() {
			return Self::find_system_by_model_id_on_connection(conn, model_id).await;
		}

		let mut q = QueryBuilder::<Postgres>::new("UPDATE model_configs SET ");
		let mut separated = q.separated(", ");

		for field in fields {
			let (col, val) = field.column_and_value();
			match val {
				ConfigValue::Text(v) => {
					separated.push(format!("{col} = ")).push_bind_unseparated(v).push_unseparated("::TEXT");
				}
				ConfigValue::Int(v) => {
					separated.push(format!("{col} = ")).push_bind_unseparated(v).push_unseparated("::INTEGER");
				}
				ConfigValue::Bool(v) => {
					separated.push(format!("{col} = ")).push_bind_unseparated(v).push_unseparated("::BOOLEAN");
				}
				ConfigValue::Json(v) => {
					separated.push(format!("{col} = ")).push_bind_unseparated(v.map(Json)).push_unseparated("::JSONB");
				}
				ConfigValue::JsonMerge(v) => {
					separated
						.push(format!("{col} = jsonb_strip_nulls({col} || "))
						.push_bind_unseparated(Json(v))
						.push_unseparated("::JSONB)");
				}
			};
		}

		drop(separated);
		q.push(
			r#"
			, updated_at = NOW()
			WHERE model_id = 
			"#,
		);
		q.push_bind(model_id);
		q.push(
			r#"
			AND owner_id IS NULL
			RETURNING
				id,
				owner_id,
				model_id,
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
		);

		q.build_query_as::<ModelConfig>().fetch_one(&mut *conn).await
	}
}
