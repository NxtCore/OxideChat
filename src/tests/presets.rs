#[cfg(test)]
mod ids {
	use crate::types::*;

	#[test]
	fn slugs_are_lowercase_parts_joined_by_slashes() {
		let long = "a".repeat(MAX_PRESET_SLUG_LEN + 1);
		let valid = ["code-reviewer", "a", "gpt5-helper-2", "ssc/test-reviewer", "team/docs/writer"];
		let invalid = ["", "-reviewer", "reviewer-", "Code", "code_reviewer", "~reviewer", "/reviewer", "ssc/", "ssc//reviewer", "ssc/-reviewer", long.as_str()];
		assert!(valid.iter().all(|slug| is_valid_preset_slug(slug)));
		assert!(invalid.iter().all(|slug| !is_valid_preset_slug(slug)));
	}

	#[test]
	fn only_the_presets_namespace_is_a_preset_key() {
		assert_eq!(preset_slug("presets/code-reviewer"), Some("code-reviewer"));
		assert_eq!(preset_slug("presets/ssc/test-reviewer"), Some("ssc/test-reviewer"));
		assert_eq!(preset_slug("presets/"), None);
		assert_eq!(preset_slug("openrouter/presets/x"), None);
		assert_eq!(preset_slug("~anthropic/claude-sonnet-latest"), None);
	}

	#[test]
	fn the_presets_namespace_is_reserved_for_models_and_providers() {
		assert!(is_reserved_model_id("presets/x"));
		assert!(is_reserved_model_id(" Presets/X"));
		assert!(!is_reserved_model_id("openrouter/presets/x"));
		assert!(!is_reserved_model_id("presets-model"));
		assert!(is_reserved_provider_name("presets"));
		assert!(is_reserved_provider_name(" PRESETS "));
		assert!(!is_reserved_provider_name("My Presets"));
	}
}

#[cfg(test)]
mod validation {
	use crate::types::*;
	use uuid::Uuid;

	pub fn request(slug: &str) -> SavePresetRequest {
		SavePresetRequest {
			slug: slug.to_string(),
			name: "Code reviewer".to_string(),
			description: None,
			icon: None,
			base_model_id: Uuid::new_v4(),
			system_prompt: None,
			sampling: PresetSampling::default(),
			reasoning: PresetReasoning::default(),
			tool_ids: Vec::new(),
			tools_locked: false,
			starters: Vec::new(),
			prompt_visible: true,
			is_enabled: true,
		}
	}

	#[test]
	fn text_is_trimmed_and_empty_values_dropped() -> Result<(), PresetError> {
		let mut input = request(" reviewer ");
		input.name = "  Reviewer ".to_string();
		input.description = Some("   ".to_string());
		input.starters = vec![" Review this ".to_string(), String::new()];
		let tool = Uuid::new_v4();
		input.tool_ids = vec![tool, tool];
		let saved = input.normalized()?;
		assert_eq!(saved.slug, "reviewer");
		assert_eq!(saved.name, "Reviewer");
		assert_eq!(saved.description, None);
		assert_eq!(saved.starters, vec!["Review this".to_string()]);
		assert_eq!(saved.tool_ids, vec![tool]);
		Ok(())
	}

	#[test]
	fn invalid_fields_are_rejected() {
		assert!(matches!(request("Bad Slug").normalized(), Err(PresetError::InvalidSlug)));

		let mut empty_name = request("ok");
		empty_name.name = " ".to_string();
		assert!(matches!(empty_name.normalized(), Err(PresetError::Invalid)));

		let mut many_starters = request("ok");
		many_starters.starters = (0..=MAX_PRESET_STARTERS).map(|n| format!("Starter {n}")).collect();
		assert!(matches!(many_starters.normalized(), Err(PresetError::Invalid)));

		let mut hot = request("ok");
		hot.sampling.temperature = Some(3.0);
		assert!(matches!(hot.normalized(), Err(PresetError::Invalid)));

		let mut effort = request("ok");
		effort.reasoning.effort = Some("extreme".to_string());
		assert!(matches!(effort.normalized(), Err(PresetError::Invalid)));
	}
}

#[cfg(test)]
mod merge {
	use crate::types::ChatSampling;
	use crate::types::PresetSampling;
	use crate::types::chat::runner::request::{SamplingLayers, merge_sampling_with_priority};
	use crate::types::models::Model;
	use crate::types::models_configs::ModelConfig;
	use serde_json::json;
	use sqlx::types::Json;
	use uuid::Uuid;

	fn model() -> Model {
		Model {
			id: Uuid::new_v4(),
			provider_id: Uuid::new_v4(),
			model_id: "test/model".to_string(),
			display_name: "Test".to_string(),
			capabilities: Json(vec![]),
			input_modalities: Json(vec![]),
			output_modalities: Json(vec![]),
			context_length: None,
			max_tokens: Some(4096),
			is_enabled: true,
			created_at: chrono::Utc::now(),
			updated_at: chrono::Utc::now(),
		}
	}

	fn config(sampling: serde_json::Value) -> Result<ModelConfig, serde_json::Error> {
		serde_json::from_value(json!({
			"id": Uuid::new_v4(),
			"owner_id": null,
			"model_id": Uuid::new_v4(),
			"stable_key": "test/model",
			"name": "Test",
			"description": null,
			"icon": null,
			"capabilities": null,
			"input_modalities": null,
			"output_modalities": null,
			"context_length": null,
			"max_output_tokens": null,
			"system_prompt": null,
			"sampling": sampling,
			"enabled_tools": [],
			"is_public": false,
			"is_featured": false,
			"is_default": false,
			"is_favorite": false,
			"category": null,
			"tags": [],
			"usage_count": 0,
			"extra_settings": {},
			"created_at": chrono::Utc::now(),
			"updated_at": chrono::Utc::now(),
		}))
	}

	#[test]
	fn the_preset_sits_between_the_chat_and_the_admin_config() -> Result<(), serde_json::Error> {
		let model = model();
		let chat: ChatSampling = serde_json::from_value(json!({"temperature": 0.3}))?;
		let preset = PresetSampling {
			temperature: Some(0.9),
			top_p: Some(0.5),
			..PresetSampling::default()
		};
		let config = config(json!({"temperature": 1.5, "top_p": 0.1, "top_k": 40, "seed": null, "stop": null}))?;
		let merged = merge_sampling_with_priority(&SamplingLayers {
			request: None,
			chat: &chat,
			preset: Some(&preset),
			config: Some(&config),
			model: &model,
		});
		assert_eq!(merged.temperature, Some(0.3));
		assert_eq!(merged.top_p, Some(0.5));
		assert_eq!(merged.top_k, Some(40));
		assert_eq!(merged.max_tokens, Some(4096));
		Ok(())
	}

	#[test]
	fn admin_sampling_applies_without_a_preset() -> Result<(), serde_json::Error> {
		let model = model();
		let chat = ChatSampling::default();
		let config = config(json!({"temperature": 0.7}))?;
		let merged = merge_sampling_with_priority(&SamplingLayers {
			request: None,
			chat: &chat,
			preset: None,
			config: Some(&config),
			model: &model,
		});
		assert_eq!(merged.temperature, Some(0.7));
		Ok(())
	}
}

#[cfg(test)]
mod repository {
	use super::validation::request;
	use crate::tests::gateway::support::{create_provider_model, create_team};
	use crate::tests::support::{TestResult, create_user};
	use crate::types::models::{Model, ModelSyncInput};
	use crate::types::*;
	use sqlx::PgPool;
	use uuid::Uuid;

	struct Fixture {
		user_id: Uuid,
		model_id: Uuid,
	}

	async fn fixture(pool: &PgPool) -> TestResult<Fixture> {
		let user_id = create_user(pool).await?;
		create_team(pool, user_id, &format!("Team {}", Uuid::new_v4()), true).await?;
		let (_, model_id) = create_provider_model(pool, &format!("Provider {}", Uuid::new_v4().simple()), "base").await?;
		Ok(Fixture { user_id, model_id })
	}

	fn for_model(slug: &str, model_id: Uuid) -> SavePresetRequest {
		SavePresetRequest { base_model_id: model_id, ..request(slug) }
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_slug_is_unique_across_users(pool: PgPool) -> TestResult {
		let first = fixture(&pool).await?;
		let second = fixture(&pool).await?;
		Preset::create_personal(&pool, &first.user_id, for_model("reviewer", first.model_id)).await?;
		let taken = Preset::create_personal(&pool, &second.user_id, for_model("reviewer", second.model_id)).await;
		assert!(matches!(taken, Err(PresetError::SlugTaken)));
		assert!(!Preset::slug_available(&pool, "reviewer", None).await?);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_slug_with_parts_resolves_and_the_database_checks_its_shape(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		Preset::create_personal(&pool, &fixture.user_id, for_model("ssc/test-reviewer", fixture.model_id)).await?;
		assert!(Preset::find_visible_by_slug(&pool, &fixture.user_id, "ssc/test-reviewer").await?.is_some());
		let bad = sqlx::query("INSERT INTO model_presets (scope, owner_id, slug, name) VALUES ('personal', $1, 'ssc//x', 'x')")
			.bind(fixture.user_id)
			.execute(&pool)
			.await;
		assert!(bad.is_err());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_deleted_preset_frees_its_slug(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let preset = Preset::create_personal(&pool, &fixture.user_id, for_model("reviewer", fixture.model_id)).await?;
		assert!(Preset::slug_available(&pool, "reviewer", Some(&preset.id)).await?);
		Preset::delete_personal(&pool, &fixture.user_id, &preset.id).await?;
		assert!(Preset::slug_available(&pool, "reviewer", None).await?);
		assert!(Preset::find_visible_by_slug(&pool, &fixture.user_id, "reviewer").await?.is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn personal_presets_are_private_to_their_owner(pool: PgPool) -> TestResult {
		let owner = fixture(&pool).await?;
		let other = fixture(&pool).await?;
		let preset = Preset::create_personal(&pool, &owner.user_id, for_model("mine", owner.model_id)).await?;
		assert!(Preset::find_visible_by_slug(&pool, &owner.user_id, "mine").await?.is_some());
		assert!(Preset::find_visible_by_slug(&pool, &other.user_id, "mine").await?.is_none());
		let edit = Preset::update_personal(&pool, &other.user_id, &preset.id, for_model("mine", other.model_id)).await;
		assert!(matches!(edit, Err(PresetError::NotFound)));
		assert!(matches!(Preset::delete_personal(&pool, &other.user_id, &preset.id).await, Err(PresetError::NotFound)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_base_model_must_be_usable(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		create_team(&pool, user_id, "Restricted", false).await?;
		let (_, model_id) = create_provider_model(&pool, "Closed", "locked").await?;
		let denied = Preset::create_personal(&pool, &user_id, for_model("locked", model_id)).await;
		assert!(matches!(denied, Err(PresetError::BaseModelNotAllowed)));
		let missing = Preset::create_personal(&pool, &user_id, for_model("missing", Uuid::new_v4())).await;
		assert!(matches!(missing, Err(PresetError::BaseModelNotAllowed)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn other_users_tools_cannot_be_bundled(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let stranger = create_user(&pool).await?;
		let tool_id: Uuid = sqlx::query_scalar(
			"INSERT INTO tools (owner_id, name, display_name, source_kind, input_schema, is_enabled) VALUES ($1, 'private', 'Private', 'HTTP', '{}', true) RETURNING id",
		)
		.bind(stranger)
		.fetch_one(&pool)
		.await?;
		let mut input = for_model("tools", fixture.model_id);
		input.tool_ids = vec![tool_id];
		assert!(matches!(Preset::create_personal(&pool, &fixture.user_id, input).await, Err(PresetError::ToolsNotAllowed)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn responses_report_a_removed_base_model(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		Preset::create_personal(&pool, &fixture.user_id, for_model("orphan", fixture.model_id)).await?;
		sqlx::query("DELETE FROM models WHERE id = $1").bind(fixture.model_id).execute(&pool).await?;
		let presets = Preset::list_for_user(&pool, &fixture.user_id).await?;
		let responses = Preset::responses_for_user(&pool, &fixture.user_id, presets).await?;
		assert_eq!(responses.len(), 1);
		assert_eq!(responses[0].key, "presets/orphan");
		assert_eq!(responses[0].unavailable, Some(PresetUnavailable::BaseModelUnavailable));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_database_refuses_the_reserved_namespace(pool: PgPool) -> TestResult {
		let provider = sqlx::query("INSERT INTO providers (kind, name, base_url) VALUES ('OPENAI', 'Presets', 'https://example.com')").execute(&pool).await;
		assert!(provider.is_err());
		let (provider_id, _) = create_provider_model(&pool, "Upstream", "real").await?;
		let model = sqlx::query("INSERT INTO models (provider_id, model_id, display_name) VALUES ($1, 'presets/x', 'x')").bind(provider_id).execute(&pool).await;
		assert!(model.is_err());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn model_sync_skips_ids_in_the_reserved_namespace(pool: PgPool) -> TestResult {
		let (provider_id, _) = create_provider_model(&pool, "Upstream", "real").await?;
		let discovered = ["real", "presets/x"].map(|id| ModelSyncInput {
			provider_id,
			model_id: id.to_string(),
			display_name: id.to_string(),
			capabilities: serde_json::json!([]),
			input_modalities: serde_json::json!(["text"]),
			output_modalities: serde_json::json!(["text"]),
			context_length: None,
			max_tokens: None,
		});
		Model::sync_provider_models(&pool, &provider_id, &discovered).await?;
		assert!(Model::find_by_model_id(&pool, "presets/x").await?.is_none());
		assert!(Model::find_by_model_id(&pool, "real").await?.is_some());
		Ok(())
	}
}
