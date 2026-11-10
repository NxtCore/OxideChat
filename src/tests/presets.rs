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
			scope: None,
			team_id: None,
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
	use crate::types::chat::runner::request::{SamplingLayers, merge_sampling_with_priority, turn_tool_ids};
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

	#[test]
	fn admin_tools_are_added_to_the_turn_tools() -> Result<(), serde_json::Error> {
		let mut config = config(json!({}))?;
		config.enabled_tools = Json(vec!["b".to_string(), "c".to_string()]);
		let turn = vec!["a".to_string(), "b".to_string()];
		assert_eq!(turn_tool_ids(&turn, Some(&config)), vec!["a", "b", "c"]);
		assert_eq!(turn_tool_ids(&turn, None), vec!["a", "b"]);
		assert_eq!(turn_tool_ids(&[], Some(&config)), vec!["b", "c"]);
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

	pub fn for_model(slug: &str, model_id: Uuid) -> SavePresetRequest {
		SavePresetRequest { base_model_id: model_id, ..request(slug) }
	}

	pub fn personal(user_id: Uuid) -> PresetActor {
		PresetActor {
			user_id,
			personal: true,
			installation_admin: false,
			admin_teams: Vec::new(),
		}
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_slug_is_unique_across_users(pool: PgPool) -> TestResult {
		let first = fixture(&pool).await?;
		let second = fixture(&pool).await?;
		Preset::create(&pool, &personal(first.user_id), for_model("reviewer", first.model_id)).await?;
		let taken = Preset::create(&pool, &personal(second.user_id), for_model("reviewer", second.model_id)).await;
		assert!(matches!(taken, Err(PresetError::SlugTaken)));
		assert!(!Preset::slug_available(&pool, "reviewer", None).await?);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_slug_with_parts_resolves_and_the_database_checks_its_shape(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		Preset::create(&pool, &personal(fixture.user_id), for_model("ssc/test-reviewer", fixture.model_id)).await?;
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
		let preset = Preset::create(&pool, &personal(fixture.user_id), for_model("reviewer", fixture.model_id)).await?;
		assert!(Preset::slug_available(&pool, "reviewer", Some(&preset.id)).await?);
		Preset::delete(&pool, &personal(fixture.user_id), &preset.id).await?;
		assert!(Preset::slug_available(&pool, "reviewer", None).await?);
		assert!(Preset::find_visible_by_slug(&pool, &fixture.user_id, "reviewer").await?.is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn personal_presets_are_private_to_their_owner(pool: PgPool) -> TestResult {
		let owner = fixture(&pool).await?;
		let other = fixture(&pool).await?;
		let preset = Preset::create(&pool, &personal(owner.user_id), for_model("mine", owner.model_id)).await?;
		assert!(Preset::find_visible_by_slug(&pool, &owner.user_id, "mine").await?.is_some());
		assert!(Preset::find_visible_by_slug(&pool, &other.user_id, "mine").await?.is_none());
		let edit = Preset::update(&pool, &personal(other.user_id), &preset.id, for_model("mine", other.model_id)).await;
		assert!(matches!(edit, Err(PresetError::NotFound)));
		assert!(matches!(Preset::delete(&pool, &personal(other.user_id), &preset.id).await, Err(PresetError::NotFound)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_base_model_must_be_usable(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		create_team(&pool, user_id, "Restricted", false).await?;
		let (_, model_id) = create_provider_model(&pool, "Closed", "locked").await?;
		let denied = Preset::create(&pool, &personal(user_id), for_model("locked", model_id)).await;
		assert!(matches!(denied, Err(PresetError::BaseModelNotAllowed)));
		let missing = Preset::create(&pool, &personal(user_id), for_model("missing", Uuid::new_v4())).await;
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
		assert!(matches!(Preset::create(&pool, &personal(fixture.user_id), input).await, Err(PresetError::ToolsNotAllowed)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn responses_report_a_removed_base_model(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		Preset::create(&pool, &personal(fixture.user_id), for_model("orphan", fixture.model_id)).await?;
		sqlx::query("DELETE FROM models WHERE id = $1").bind(fixture.model_id).execute(&pool).await?;
		let presets = Preset::list_for_user(&pool, &fixture.user_id).await?;
		let responses = Preset::responses_for_user(&pool, &personal(fixture.user_id), presets).await?;
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

#[cfg(test)]
mod scopes {
	use super::repository::{for_model, personal};
	use crate::tests::gateway::support::{create_provider_model, create_team};
	use crate::tests::support::{TestResult, create_user};
	use crate::types::*;
	use sqlx::PgPool;
	use uuid::Uuid;

	pub struct Team {
		pub team_id: Uuid,
		pub admin_id: Uuid,
		pub member_id: Uuid,
		pub outsider_id: Uuid,
		pub outsider_team_id: Uuid,
		pub model_id: Uuid,
	}

	pub async fn team(pool: &PgPool) -> TestResult<Team> {
		let admin_id = create_user(pool).await?;
		let member_id = create_user(pool).await?;
		let outsider_id = create_user(pool).await?;
		let team_id = create_team(pool, admin_id, &format!("Team {}", Uuid::new_v4()), true).await?;
		sqlx::query("UPDATE team_members SET role = 'admin' WHERE team_id = $1 AND user_id = $2")
			.bind(team_id)
			.bind(admin_id)
			.execute(pool)
			.await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)").bind(team_id).bind(member_id).execute(pool).await?;
		let outsider_team_id = create_team(pool, outsider_id, &format!("Other {}", Uuid::new_v4()), true).await?;
		let (_, model_id) = create_provider_model(pool, &format!("Provider {}", Uuid::new_v4().simple()), "base").await?;
		Ok(Team {
			team_id,
			admin_id,
			member_id,
			outsider_id,
			outsider_team_id,
			model_id,
		})
	}

	pub async fn actor(pool: &PgPool, user_id: Uuid, installation_admin: bool) -> TestResult<PresetActor> {
		Ok(PresetActor::load(pool, &user_id, true, installation_admin).await?)
	}

	pub fn shared(slug: &str, model_id: Uuid, scope: PresetScope, team_id: Option<Uuid>) -> SavePresetRequest {
		SavePresetRequest {
			scope: Some(scope),
			team_id,
			..for_model(slug, model_id)
		}
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_presets_are_managed_by_team_admins_and_seen_by_members(pool: PgPool) -> TestResult {
		let team = team(&pool).await?;
		let member = actor(&pool, team.member_id, false).await?;
		let denied = Preset::create(&pool, &member, shared("team-x", team.model_id, PresetScope::Team, Some(team.team_id))).await;
		assert!(matches!(denied, Err(PresetError::Forbidden)));

		let admin = actor(&pool, team.admin_id, false).await?;
		let preset = Preset::create(&pool, &admin, shared("team-x", team.model_id, PresetScope::Team, Some(team.team_id))).await?;
		assert_eq!(preset.owner_id, None);
		assert_eq!(preset.team_id, Some(team.team_id));

		assert!(Preset::find_visible_by_slug(&pool, &team.member_id, "team-x").await?.is_some());
		assert!(Preset::find_visible_by_slug(&pool, &team.outsider_id, "team-x").await?.is_none());
		let responses = Preset::responses_for_user(&pool, &member, Preset::list_for_user(&pool, &team.member_id).await?).await?;
		assert_eq!(responses.len(), 1);
		assert!(!responses[0].can_edit);
		assert_eq!(responses[0].team.as_ref().map(|team| team.id), Some(team.team_id));

		let edit = Preset::update(&pool, &member, &preset.id, for_model("team-x", team.model_id)).await;
		assert!(matches!(edit, Err(PresetError::NotFound)));
		let renamed = Preset::update(&pool, &admin, &preset.id, for_model("team-y", team.model_id)).await?;
		assert_eq!(renamed.scope(), PresetScope::Team);
		assert!(matches!(Preset::delete(&pool, &member, &preset.id).await, Err(PresetError::NotFound)));
		Preset::delete(&pool, &admin, &preset.id).await?;
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn installation_presets_need_the_admin_permission_and_everyone_sees_them(pool: PgPool) -> TestResult {
		let team = team(&pool).await?;
		let admin = actor(&pool, team.admin_id, false).await?;
		let denied = Preset::create(&pool, &admin, shared("org", team.model_id, PresetScope::Installation, None)).await;
		assert!(matches!(denied, Err(PresetError::Forbidden)));
		let installation = actor(&pool, team.admin_id, true).await?;
		Preset::create(&pool, &installation, shared("org", team.model_id, PresetScope::Installation, None)).await?;
		for user in [team.member_id, team.outsider_id] {
			assert!(Preset::find_visible_by_slug(&pool, &user, "org").await?.is_some());
		}
		let other_team = Preset::create(&pool, &installation, shared("other-team", team.model_id, PresetScope::Team, Some(team.outsider_team_id))).await;
		assert!(other_team.is_ok());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn shared_presets_only_bundle_system_tools(pool: PgPool) -> TestResult {
		let team = team(&pool).await?;
		let own_tool: Uuid = sqlx::query_scalar(
			"INSERT INTO tools (owner_id, name, display_name, source_kind, input_schema, is_enabled) VALUES ($1, 'mine', 'Mine', 'HTTP', '{}', true) RETURNING id",
		)
		.bind(team.admin_id)
		.fetch_one(&pool)
		.await?;
		let system_tool: Uuid =
			sqlx::query_scalar("INSERT INTO tools (owner_id, name, display_name, source_kind, input_schema, is_enabled) VALUES (NULL, 'sys', 'Sys', 'HTTP', '{}', true) RETURNING id")
				.fetch_one(&pool)
				.await?;
		let admin = actor(&pool, team.admin_id, false).await?;
		let mut with_own = shared("tools-own", team.model_id, PresetScope::Team, Some(team.team_id));
		with_own.tool_ids = vec![own_tool];
		assert!(matches!(Preset::create(&pool, &admin, with_own).await, Err(PresetError::ToolsNotAllowed)));
		let mut with_system = shared("tools-sys", team.model_id, PresetScope::Team, Some(team.team_id));
		with_system.tool_ids = vec![system_tool];
		assert!(Preset::create(&pool, &admin, with_system).await.is_ok());
		let mut personal_own = for_model("tools-personal", team.model_id);
		personal_own.tool_ids = vec![own_tool];
		assert!(Preset::create(&pool, &personal(team.admin_id), personal_own).await.is_ok());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_slug_is_unique_across_scopes(pool: PgPool) -> TestResult {
		let team = team(&pool).await?;
		let installation = actor(&pool, team.admin_id, true).await?;
		Preset::create(&pool, &personal(team.member_id), for_model("same", team.model_id)).await?;
		for (scope, team_id) in [(PresetScope::Team, Some(team.team_id)), (PresetScope::Installation, None)] {
			let taken = Preset::create(&pool, &installation, shared("same", team.model_id, scope, team_id)).await;
			assert!(matches!(taken, Err(PresetError::SlugTaken)));
		}
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn admins_rename_and_disable_any_preset(pool: PgPool) -> TestResult {
		let team = team(&pool).await?;
		let preset = Preset::create(&pool, &personal(team.member_id), for_model("squatted", team.model_id)).await?;
		let request = AdminUpdatePresetRequest {
			slug: Some("released".to_string()),
			is_enabled: Some(false),
		};
		let updated = Preset::admin_update(&pool, &team.admin_id, &preset.id, request).await?;
		assert_eq!(updated.key, "presets/released");
		assert!(!updated.is_enabled);
		assert_eq!(updated.owner.map(|owner| owner.id), Some(team.member_id));
		assert!(Preset::slug_available(&pool, "squatted", None).await?);
		let filtered = Preset::admin_list(&pool, &AdminPresetQuery { scope: Some(PresetScope::Team), team_id: None }).await?;
		assert!(filtered.is_empty());
		Ok(())
	}
}

#[cfg(test)]
mod gateway {
	use super::repository::{for_model, personal};
	use super::scopes::{actor, shared, team};
	use crate::tests::budgets::support::{assign, create_budget, spend};
	use crate::tests::gateway::support::insert_key;
	use crate::tests::support::TestResult;
	use crate::types::*;
	use omniference::types::providers::openai::{OpenAIChatRequest, OpenAIMessageContent};
	use serde_json::json;
	use sqlx::PgPool;
	use uuid::Uuid;

	async fn context(pool: &PgPool, user_id: Option<Uuid>, team_id: Uuid) -> TestResult<GatewayAuthContext> {
		let (_, token) = insert_key(pool, user_id, team_id, json!(["inference:read", "inference:write"])).await?;
		Ok(GatewayCredential::authenticate(pool, &token).await?)
	}

	async fn listed(pool: &PgPool, context: &GatewayAuthContext) -> TestResult<Vec<String>> {
		let mut ids: Vec<String> = GatewayModel::list_for_context(pool, context)
			.await?
			.into_iter()
			.map(|model| model.id)
			.filter(|id| id.starts_with(PRESET_KEY_PREFIX))
			.collect();
		ids.sort();
		Ok(ids)
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn models_list_the_presets_each_key_type_can_call(pool: PgPool) -> TestResult {
		let team = team(&pool).await?;
		let installation = actor(&pool, team.admin_id, true).await?;
		Preset::create(&pool, &personal(team.member_id), for_model("member-own", team.model_id)).await?;
		Preset::create(&pool, &personal(team.admin_id), for_model("admin-own", team.model_id)).await?;
		Preset::create(&pool, &installation, shared("team-shared", team.model_id, PresetScope::Team, Some(team.team_id))).await?;
		Preset::create(&pool, &installation, shared("org-wide", team.model_id, PresetScope::Installation, None)).await?;
		let off = Preset::create(&pool, &installation, shared("org-off", team.model_id, PresetScope::Installation, None)).await?;
		let disable = AdminUpdatePresetRequest { slug: None, is_enabled: Some(false) };
		Preset::admin_update(&pool, &team.admin_id, &off.id, disable).await?;

		let member_key = context(&pool, Some(team.member_id), team.team_id).await?;
		assert_eq!(listed(&pool, &member_key).await?, ["presets/member-own", "presets/org-wide", "presets/team-shared"]);
		let service_key = context(&pool, None, team.team_id).await?;
		assert_eq!(listed(&pool, &service_key).await?, ["presets/org-wide", "presets/team-shared"]);
		let outsider_key = context(&pool, Some(team.outsider_id), team.outsider_team_id).await?;
		assert_eq!(listed(&pool, &outsider_key).await?, ["presets/org-wide"]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn another_users_personal_preset_is_not_found(pool: PgPool) -> TestResult {
		let team = team(&pool).await?;
		Preset::create(&pool, &personal(team.admin_id), for_model("admin-own", team.model_id)).await?;
		let member_key = context(&pool, Some(team.member_id), team.team_id).await?;
		let resolved = GatewayModel::resolve_inference(&pool, &member_key, "presets/admin-own").await;
		assert!(matches!(resolved, Err(GatewayModelAccessError::NotFound)));
		let service_key = context(&pool, None, team.team_id).await?;
		assert!(GatewayModel::resolve_accessible(&pool, &service_key, "presets/admin-own").await?.is_none());
		let admin_key = context(&pool, Some(team.admin_id), team.team_id).await?;
		assert_eq!(GatewayModel::resolve_accessible(&pool, &admin_key, "presets/admin-own").await?, Some(team.model_id));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_preset_never_grants_access_to_its_base_model(pool: PgPool) -> TestResult {
		let team = team(&pool).await?;
		let installation = actor(&pool, team.admin_id, true).await?;
		Preset::create(&pool, &installation, shared("org-wide", team.model_id, PresetScope::Installation, None)).await?;
		sqlx::query("UPDATE teams SET allow_all_models = false WHERE id = $1").bind(team.team_id).execute(&pool).await?;
		let member_key = context(&pool, Some(team.member_id), team.team_id).await?;
		assert!(listed(&pool, &member_key).await?.is_empty());
		assert!(GatewayModel::resolve_accessible(&pool, &member_key, "presets/org-wide").await?.is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_preset_prompt_comes_first_and_sampling_fills_only_empty_fields(pool: PgPool) -> TestResult {
		let team = team(&pool).await?;
		let mut request = for_model("tuned", team.model_id);
		request.system_prompt = Some("Be brief.".to_string());
		request.sampling = PresetSampling {
			temperature: Some(0.2),
			top_p: Some(0.5),
			..PresetSampling::default()
		};
		request.reasoning.effort = Some("low".to_string());
		Preset::create(&pool, &personal(team.member_id), request).await?;
		let member_key = context(&pool, Some(team.member_id), team.team_id).await?;
		let inference = GatewayModel::resolve_inference(&pool, &member_key, "presets/tuned").await?;
		assert_eq!(inference.model_id, team.model_id);
		assert_eq!(inference.public_model_id, "presets/tuned");

		let mut chat: OpenAIChatRequest = serde_json::from_value(json!({
			"model": "presets/tuned",
			"messages": [{"role": "user", "content": "Hi"}],
			"temperature": 0.9
		}))?;
		inference.apply_preset_to_chat(&mut chat);
		assert_eq!(chat.messages.len(), 2);
		assert_eq!(chat.messages[0].role, "system");
		assert!(matches!(&chat.messages[0].content, Some(OpenAIMessageContent::Text(text)) if text == "Be brief."));
		assert_eq!(chat.temperature, Some(0.9));
		assert_eq!(chat.top_p, Some(0.5));
		assert!(chat.reasoning_effort.is_some());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn budget_is_reserved_against_the_base_model(pool: PgPool) -> TestResult {
		let fixture = crate::tests::budgets::support::fixture(&pool, 1_000_000.0).await?;
		Preset::create(&pool, &personal(fixture.user_id), for_model("paid-preset", fixture.model_id)).await?;
		let budget = create_budget(&pool, "10", "block").await?;
		assign(&pool, budget, Some(fixture.user_id), None, None).await?;
		let ledger = BudgetLedger::spawn();
		let context = context(&pool, Some(fixture.user_id), fixture.team_id).await?;
		let tokens = TokenEstimate { input: 0, output: 1 };
		let inference = GatewayModel::resolve_inference(&pool, &context, "presets/paid-preset").await?;
		let admitted = GatewayModel::authorize_inference(&pool, &ledger, &context, inference, tokens).await?;
		assert_eq!(admitted.model_id, fixture.model_id);
		assert!(admitted.budget.reservation.is_some());
		drop(admitted);
		spend(&pool, &fixture, None, 10).await?;
		let inference = GatewayModel::resolve_inference(&pool, &context, "presets/paid-preset").await?;
		let blocked = GatewayModel::authorize_inference(&pool, &ledger, &context, inference, tokens).await;
		assert!(matches!(blocked, Err(GatewayModelAccessError::BudgetExceeded)));
		Ok(())
	}
}
