#[cfg(test)]
mod tests {
	use crate::tests::support::paged;
	use crate::types::models::{Model, ModelListParams, ModelSyncInput, ModelViewer};
	use crate::types::models_configs::ModelConfig;
	use crate::types::teams::Team;
	use serde_json::json;
	use crate::tests::support::insert_user;
	use sqlx::PgPool;
	use sqlx::types::Json;
	use uuid::Uuid;

	async fn create_user(pool: &PgPool, suffix: &str) -> Uuid {
		let user_id = insert_user(pool, &format!("user_{suffix}")).await.unwrap();
		Team::ensure_default_membership(pool, &user_id).await.unwrap();
		user_id
	}

	async fn create_provider(pool: &PgPool, name: &str, is_enabled: bool) -> Uuid {
		sqlx::query_scalar::<_, Uuid>(
			r#"
			INSERT INTO providers (kind, name, base_url, is_enabled)
			VALUES ('OPENAI', $1, 'https://example.com/v1', $2)
			RETURNING id
			"#,
		)
		.bind(name)
		.bind(is_enabled)
		.fetch_one(pool)
		.await
		.unwrap()
	}

	async fn create_model(pool: &PgPool, provider_id: Uuid, model_id: &str, display_name: &str, is_enabled: bool) -> Uuid {
		sqlx::query_scalar::<_, Uuid>(
			r#"
			INSERT INTO models (
				provider_id,
				model_id,
				display_name,
				capabilities,
				input_modalities,
				output_modalities,
				context_length,
				max_tokens,
				is_enabled
			)
			VALUES ($1, $2, $3, $4, $5, $6, 100, 10, $7)
			RETURNING id
			"#,
		)
		.bind(provider_id)
		.bind(model_id)
		.bind(display_name)
		.bind(Json(vec!["model".to_string()]))
		.bind(Json(vec!["text".to_string()]))
		.bind(Json(vec!["text".to_string()]))
		.bind(is_enabled)
		.fetch_one(pool)
		.await
		.unwrap()
	}

	#[allow(clippy::too_many_arguments)]
	async fn create_config(pool: &PgPool, owner_id: Option<Uuid>, model_id: Uuid, stable_key: &str, name: &str, capability: &str, icon: Option<&str>, is_favorite: bool) {
		sqlx::query(
			r#"
			INSERT INTO model_configs (
				owner_id,
				model_id,
				stable_key,
				name,
				icon,
				capabilities,
				input_modalities,
				output_modalities,
				context_length,
				max_output_tokens,
				is_favorite,
				tags
			)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 200, 20, $9, $10)
			"#,
		)
		.bind(owner_id)
		.bind(model_id)
		.bind(stable_key)
		.bind(name)
		.bind(icon)
		.bind(Json(vec![capability.to_string()]))
		.bind(Json(vec!["text".to_string(), "image".to_string()]))
		.bind(Json(vec!["text".to_string()]))
		.bind(is_favorite)
		.bind(Json(Vec::<String>::new()))
		.execute(pool)
		.await
		.unwrap();
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn user_model_list_resolves_user_system_and_model_fallbacks(pool: PgPool) {
		let user_id = create_user(&pool, "owner").await;
		let other_user_id = create_user(&pool, "other").await;
		let provider_id = create_provider(&pool, "Provider", true).await;
		let model_id = create_model(&pool, provider_id, "model-one", "Model One", true).await;

		create_config(&pool, None, model_id, "system:model-one", "System Model", "system", Some("system-icon"), true).await;
		create_config(&pool, Some(user_id), model_id, "user:model-one", "User Model", "user", Some("user-icon"), true).await;

		let viewer = ModelViewer { user_id: &user_id };
		let response = Model::list_for_user(&pool, viewer, &paged(1, 10), false, None, false, None).await.unwrap();
		assert_eq!(response.items.len(), 1);
		assert_eq!(response.items[0].capabilities, vec!["user"]);
		assert_eq!(response.items[0].icon.as_deref(), Some("user-icon"));
		assert!(response.items[0].is_favorite);

		let other_viewer = ModelViewer { user_id: &other_user_id };
		let response = Model::list_for_user(&pool, other_viewer, &paged(1, 10), false, None, false, None).await.unwrap();
		assert_eq!(response.items.len(), 1);
		assert_eq!(response.items[0].capabilities, vec!["system"]);
		assert_eq!(response.items[0].icon.as_deref(), Some("system-icon"));
		assert!(!response.items[0].is_favorite);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn show_disabled_controls_model_and_provider_filtering(pool: PgPool) {
		let user_id = create_user(&pool, "disabled").await;
		let enabled_provider_id = create_provider(&pool, "Enabled Provider", true).await;
		let disabled_provider_id = create_provider(&pool, "Disabled Provider", false).await;
		create_model(&pool, enabled_provider_id, "disabled-model", "Disabled Model", false).await;
		create_model(&pool, disabled_provider_id, "provider-disabled-model", "Provider Disabled Model", true).await;

		let viewer = ModelViewer { user_id: &user_id };
		let hidden = Model::list_for_user(&pool, viewer, &paged(1, 10), false, None, false, None).await.unwrap();
		assert!(hidden.items.is_empty());

		let viewer = ModelViewer { user_id: &user_id };
		let visible = Model::list_for_user(&pool, viewer, &paged(1, 10), true, None, false, None).await.unwrap();
		assert_eq!(visible.items.len(), 2);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn admin_search_escapes_like_wildcards(pool: PgPool) {
		let provider_id = create_provider(&pool, "Search Provider", true).await;
		create_model(&pool, provider_id, "percent", "100% Real", true).await;
		create_model(&pool, provider_id, "percent-decoy", "1000 Real", true).await;
		create_model(&pool, provider_id, "underscore", "under_score", true).await;
		create_model(&pool, provider_id, "underscore-decoy", "underXscore", true).await;
		create_model(&pool, provider_id, "slash", r"slash\name", true).await;
		create_model(&pool, provider_id, "slash-decoy", "slash-name", true).await;

		let percent = Model::list_for_admin(&pool, &paged(1, 10), &search("100%")).await.unwrap();
		assert_eq!(percent.items.len(), 1);
		assert_eq!(percent.items[0].display_name, "100% Real");

		let underscore = Model::list_for_admin(&pool, &paged(1, 10), &search("under_")).await.unwrap();
		assert_eq!(underscore.items.len(), 1);
		assert_eq!(underscore.items[0].display_name, "under_score");

		let slash = Model::list_for_admin(&pool, &paged(1, 10), &search(r"slash\")).await.unwrap();
		assert_eq!(slash.items.len(), 1);
		assert_eq!(slash.items[0].display_name, r"slash\name");
	}

	fn search(query: &str) -> ModelListParams {
		ModelListParams { query: Some(query.to_string()), ..ModelListParams::default() }
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn admin_list_filters_by_provider_and_output(pool: PgPool) {
		let first = create_provider(&pool, "First Provider", true).await;
		let second = create_provider(&pool, "Second Provider", true).await;
		create_model(&pool, first, "text-a", "Text A", true).await;
		let image = create_model(&pool, first, "image-a", "Image A", true).await;
		create_model(&pool, second, "text-b", "Text B", true).await;
		sqlx::query("UPDATE models SET output_modalities = '[\"text\", \"image\"]' WHERE id = $1")
			.bind(image)
			.execute(&pool)
			.await
			.unwrap();

		let by_provider = ModelListParams { provider_id: Some(first), ..ModelListParams::default() };
		let names: Vec<String> = Model::list_for_admin(&pool, &paged(1, 10), &by_provider).await.unwrap().items.into_iter().map(|m| m.display_name).collect();
		assert_eq!(names, ["Image A", "Text A"]);

		let text_only = ModelListParams { output_only: Some("text".to_string()), ..ModelListParams::default() };
		let names: Vec<String> = Model::list_for_admin(&pool, &paged(1, 10), &text_only).await.unwrap().items.into_iter().map(|m| m.display_name).collect();
		assert_eq!(names, ["Text A", "Text B"]);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn pagination_uses_extra_row_for_has_more(pool: PgPool) {
		let provider_id = create_provider(&pool, "Pagination Provider", true).await;
		create_model(&pool, provider_id, "page-a", "Page A", true).await;
		create_model(&pool, provider_id, "page-b", "Page B", true).await;
		create_model(&pool, provider_id, "page-c", "Page C", true).await;

		let first = Model::list_for_admin(&pool, &paged(1, 2), &ModelListParams::default()).await.unwrap();
		assert_eq!(first.items.len(), 2);
		assert!(first.has_more);

		let second = Model::list_for_admin(&pool, &paged(2, 2), &ModelListParams::default()).await.unwrap();
		assert_eq!(second.items.len(), 1);
		assert!(!second.has_more);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn patch_name_can_sync_model_config_name(pool: PgPool) {
		let provider_id = create_provider(&pool, "Patch Provider", true).await;
		let model_id = create_model(&pool, provider_id, "patch-model", "Old Name", true).await;
		let mut tx = pool.begin().await.unwrap();

		Model::patch(&mut tx, &model_id, Some("New Name"), None).await.unwrap();
		let (display_name, stable_key) = Model::find_name_and_model_id(&mut *tx, &model_id).await.unwrap().unwrap();
		ModelConfig::ensure_system_config(&mut tx, &model_id, &stable_key, &display_name).await.unwrap();
		tx.commit().await.unwrap();

		let config_name = sqlx::query_scalar::<_, String>("SELECT name FROM model_configs WHERE model_id = $1 AND owner_id IS NULL")
			.bind(model_id)
			.fetch_one(&pool)
			.await
			.unwrap();
		assert_eq!(config_name, "New Name");
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn user_search_filters_by_display_name_and_model_id(pool: PgPool) {
		let user_id = create_user(&pool, "search").await;
		let provider_id = create_provider(&pool, "Search Test Provider", true).await;
		create_model(&pool, provider_id, "gpt-4o", "GPT 4o", true).await;
		create_model(&pool, provider_id, "claude-sonnet", "Claude Sonnet", true).await;
		create_model(&pool, provider_id, "gpt-4o-mini", "GPT 4o Mini", true).await;

		let viewer = ModelViewer { user_id: &user_id };

		let by_display = Model::list_for_user(&pool, viewer, &paged(1, 10), false, Some("GPT"), false, None).await.unwrap();
		assert_eq!(by_display.items.len(), 2);

		let viewer = ModelViewer { user_id: &user_id };
		let by_model_id = Model::list_for_user(&pool, viewer, &paged(1, 10), false, Some("claude"), false, None).await.unwrap();
		assert_eq!(by_model_id.items.len(), 1);
		assert_eq!(by_model_id.items[0].model_id, "claude-sonnet");
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn favorites_only_returns_user_favorites(pool: PgPool) {
		let user_id = create_user(&pool, "fav").await;
		let provider_id = create_provider(&pool, "Favorites Provider", true).await;
		let fav_model = create_model(&pool, provider_id, "fav-model", "Favorite Model", true).await;
		create_model(&pool, provider_id, "normal-model", "Normal Model", true).await;

		create_config(&pool, Some(user_id), fav_model, "user:fav-model", "User Favorite", "fav", None, true).await;

		let viewer = ModelViewer { user_id: &user_id };
		let favorites = Model::list_for_user(&pool, viewer, &paged(1, 10), false, None, true, None).await.unwrap();
		assert_eq!(favorites.items.len(), 1);
		assert_eq!(favorites.items[0].model_id, "fav-model");
		assert!(favorites.items[0].is_favorite);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn provider_id_filter_returns_only_matching_provider(pool: PgPool) {
		let user_id = create_user(&pool, "provider").await;
		let provider_a = create_provider(&pool, "Provider A", true).await;
		let provider_b = create_provider(&pool, "Provider B", true).await;
		create_model(&pool, provider_a, "model-a1", "Model A1", true).await;
		create_model(&pool, provider_a, "model-a2", "Model A2", true).await;
		create_model(&pool, provider_b, "model-b1", "Model B1", true).await;

		let viewer = ModelViewer { user_id: &user_id };
		let filtered = Model::list_for_user(&pool, viewer, &paged(1, 10), false, None, false, Some(&provider_a)).await.unwrap();
		assert_eq!(filtered.items.len(), 2);
		for item in &filtered.items {
			assert_eq!(item.provider.name, "Provider A");
		}
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn list_providers_returns_distinct_ids_and_names(pool: PgPool) {
		let user_id = create_user(&pool, "providers").await;
		let provider_a = create_provider(&pool, "Provider Alpha", true).await;
		let provider_b = create_provider(&pool, "Provider Beta", true).await;
		create_provider(&pool, "Disabled Provider", false).await;
		create_model(&pool, provider_a, "a1", "A1", true).await;
		create_model(&pool, provider_a, "a2", "A2", true).await;
		create_model(&pool, provider_b, "b1", "B1", true).await;

		let viewer = ModelViewer { user_id: &user_id };
		let providers = Model::list_providers_for_user(&pool, viewer).await.unwrap();
		assert_eq!(providers.len(), 2);

		let provider_names: Vec<&str> = providers.iter().map(|p| p.name.as_str()).collect();
		assert!(provider_names.contains(&"Provider Alpha"));
		assert!(provider_names.contains(&"Provider Beta"));
		assert!(!provider_names.contains(&"Disabled Provider"));
	}

	async fn restrict_general_team(pool: &PgPool) {
		sqlx::query("UPDATE teams SET allow_all_models = false WHERE is_default = true")
			.execute(pool)
			.await
			.unwrap();
	}

	async fn create_team(pool: &PgPool, name: &str) -> Uuid {
		sqlx::query_scalar::<_, Uuid>(
			r#"
			INSERT INTO teams (name, allow_all_models)
			VALUES ($1, false)
			RETURNING id
			"#,
		)
		.bind(name)
		.fetch_one(pool)
		.await
		.unwrap()
	}

	async fn add_user_to_team(pool: &PgPool, user_id: Uuid, team_id: Uuid) {
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
			.bind(team_id)
			.bind(user_id)
			.execute(pool)
			.await
			.unwrap();
	}

	async fn grant_model(pool: &PgPool, team_id: Uuid, model_id: Uuid) {
		sqlx::query("INSERT INTO team_model_access (team_id, model_id) VALUES ($1, $2)")
			.bind(team_id)
			.bind(model_id)
			.execute(pool)
			.await
			.unwrap();
	}

	async fn grant_provider(pool: &PgPool, team_id: Uuid, provider_id: Uuid) {
		sqlx::query("INSERT INTO team_model_access (team_id, provider_id) VALUES ($1, $2)")
			.bind(team_id)
			.bind(provider_id)
			.execute(pool)
			.await
			.unwrap();
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_model_access_aggregates_across_memberships(pool: PgPool) {
		restrict_general_team(&pool).await;
		let user_id = create_user(&pool, "team-union").await;
		let provider_id = create_provider(&pool, "Union Provider", true).await;
		let gpt_model = create_model(&pool, provider_id, "gpt-team", "GPT Team", true).await;
		let claude_model = create_model(&pool, provider_id, "claude-team", "Claude Team", true).await;
		create_model(&pool, provider_id, "hidden-team", "Hidden Team", true).await;

		let team_a = create_team(&pool, "Team A").await;
		let team_b = create_team(&pool, "Team B").await;
		add_user_to_team(&pool, user_id, team_a).await;
		add_user_to_team(&pool, user_id, team_b).await;
		grant_model(&pool, team_a, gpt_model).await;
		grant_model(&pool, team_b, claude_model).await;

		let viewer = ModelViewer { user_id: &user_id };
		let response = Model::list_for_user(&pool, viewer, &paged(1, 10), false, None, false, None).await.unwrap();
		let ids: Vec<&str> = response.items.iter().map(|item| item.model_id.as_str()).collect();
		assert_eq!(response.items.len(), 2);
		assert!(ids.contains(&"gpt-team"));
		assert!(ids.contains(&"claude-team"));
		assert!(!ids.contains(&"hidden-team"));
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_grants_do_not_expose_disabled_models_or_providers(pool: PgPool) {
		restrict_general_team(&pool).await;
		let user_id = create_user(&pool, "team-disabled").await;
		let disabled_provider_id = create_provider(&pool, "Disabled Team Provider", false).await;
		let enabled_provider_id = create_provider(&pool, "Enabled Team Provider", true).await;
		let provider_disabled_model = create_model(&pool, disabled_provider_id, "provider-off", "Provider Off", true).await;
		let disabled_model = create_model(&pool, enabled_provider_id, "model-off", "Model Off", false).await;
		let team_id = create_team(&pool, "Disabled Grant Team").await;
		add_user_to_team(&pool, user_id, team_id).await;
		grant_model(&pool, team_id, provider_disabled_model).await;
		grant_model(&pool, team_id, disabled_model).await;

		let viewer = ModelViewer { user_id: &user_id };
		let response = Model::list_for_user(&pool, viewer, &paged(1, 10), false, None, false, None).await.unwrap();
		assert!(response.items.is_empty());
		assert!(!Model::can_user_use_model(&pool, &user_id, &provider_disabled_model).await.unwrap());
		assert!(!Model::can_user_use_model(&pool, &user_id, &disabled_model).await.unwrap());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn provider_level_team_grant_exposes_provider_models(pool: PgPool) {
		restrict_general_team(&pool).await;
		let user_id = create_user(&pool, "team-provider").await;
		let provider_a = create_provider(&pool, "Granted Provider", true).await;
		let provider_b = create_provider(&pool, "Other Provider", true).await;
		create_model(&pool, provider_a, "provider-a-one", "Provider A One", true).await;
		create_model(&pool, provider_a, "provider-a-two", "Provider A Two", true).await;
		create_model(&pool, provider_b, "provider-b-one", "Provider B One", true).await;
		let team_id = create_team(&pool, "Provider Grant Team").await;
		add_user_to_team(&pool, user_id, team_id).await;
		grant_provider(&pool, team_id, provider_a).await;

		let viewer = ModelViewer { user_id: &user_id };
		let response = Model::list_for_user(&pool, viewer, &paged(1, 10), false, None, false, None).await.unwrap();
		let ids: Vec<&str> = response.items.iter().map(|item| item.model_id.as_str()).collect();
		assert_eq!(response.items.len(), 2);
		assert!(ids.contains(&"provider-a-one"));
		assert!(ids.contains(&"provider-a-two"));
		assert!(!ids.contains(&"provider-b-one"));
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn direct_model_authorization_requires_team_grant(pool: PgPool) {
		restrict_general_team(&pool).await;
		let user_id = create_user(&pool, "team-authz").await;
		let provider_id = create_provider(&pool, "Authz Provider", true).await;
		let model_id = create_model(&pool, provider_id, "authz-model", "Authz Model", true).await;
		assert!(!Model::can_user_use_model(&pool, &user_id, &model_id).await.unwrap());

		let team_id = create_team(&pool, "Authz Team").await;
		add_user_to_team(&pool, user_id, team_id).await;
		grant_model(&pool, team_id, model_id).await;
		assert!(Model::can_user_use_model(&pool, &user_id, &model_id).await.unwrap());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn model_assignments_replace_team_grants_and_mark_inherited_access(pool: PgPool) {
		use crate::types::AssignmentKind;
		restrict_general_team(&pool).await;
		let admin = create_user(&pool, "assign-admin").await;
		let provider_id = create_provider(&pool, "Assign Provider", true).await;
		let model_id = create_model(&pool, provider_id, "assign-model", "Assign Model", true).await;
		let direct = create_team(&pool, "Direct").await;
		let dropped = create_team(&pool, "Dropped").await;
		let through_provider = create_team(&pool, "Through Provider").await;
		grant_model(&pool, dropped, model_id).await;
		grant_provider(&pool, through_provider, provider_id).await;

		Model::replace_assignments(&pool, &admin, &model_id, &[direct]).await.unwrap();

		let response = Model::assignments(&pool, &model_id).await.unwrap();
		assert_eq!(response.kinds, vec![AssignmentKind::Team]);
		let targets: Vec<(Option<Uuid>, bool)> = response.targets.iter().map(|target| (target.id, target.inherited)).collect();
		assert_eq!(targets, vec![(Some(direct), false), (Some(through_provider), true)]);
		let audited: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE event = 'team_models_updated' AND target_id = ANY($1)")
			.bind(vec![direct, dropped])
			.fetch_one(&pool)
			.await
			.unwrap();
		assert_eq!(audited, 2);
		let metadata: serde_json::Value = sqlx::query_scalar("SELECT metadata FROM audit_logs WHERE event = 'team_models_updated' AND target_id = $1")
			.bind(direct)
			.fetch_one(&pool)
			.await
			.unwrap();
		assert_eq!(metadata["before"]["model_ids"], serde_json::json!([]));
		assert_eq!(metadata["after"]["model_ids"], serde_json::json!([model_id]));
		let metadata: serde_json::Value = sqlx::query_scalar("SELECT metadata FROM audit_logs WHERE event = 'team_models_updated' AND target_id = $1")
			.bind(dropped)
			.fetch_one(&pool)
			.await
			.unwrap();
		assert_eq!(metadata["before"]["model_ids"], serde_json::json!([model_id]));
		assert_eq!(metadata["after"]["model_ids"], serde_json::json!([]));

		assert!(Model::replace_assignments(&pool, &admin, &model_id, &[Uuid::new_v4()]).await.is_err());
		assert_eq!(Model::assignments(&pool, &model_id).await.unwrap().targets.len(), 2);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn sync_preserves_model_id_when_a_model_temporarily_disappears(pool: PgPool) {
		let provider_id = create_provider(&pool, "Sync Provider", true).await;
		let model_id = create_model(&pool, provider_id, "image-model", "Image Model", true).await;

		let removed = Model::sync_provider_models(&pool, &provider_id, &[]).await.unwrap();
		assert_eq!(removed.removed, 1);
		let disabled = Model::find_by_id(&pool, &model_id).await.unwrap().unwrap();
		assert!(!disabled.is_enabled);

		let discovered = [ModelSyncInput {
			provider_id,
			model_id: "image-model".to_string(),
			display_name: "Image Model".to_string(),
			capabilities: json!(["IMAGE_GENERATION"]),
			input_modalities: json!(["TEXT"]),
			output_modalities: json!(["IMAGE"]),
			context_length: None,
			max_tokens: None,
		}];
		let restored = Model::sync_provider_models(&pool, &provider_id, &discovered).await.unwrap();
		assert_eq!(restored.added, 0);

		let reloaded = Model::find_by_id(&pool, &model_id).await.unwrap().unwrap();
		assert_eq!(reloaded.id, model_id);
		assert!(reloaded.is_enabled);
	}

	async fn patch_icon(state: &std::sync::Arc<crate::types::JobState>, user: &crate::types::User, model_id: Uuid, icon: &str) -> axum::response::Response {
		use axum::extract::{Path, State};
		use axum::response::IntoResponse;
		let body: crate::types::models::AdminModelPatchBody = serde_json::from_value(json!({"icon": icon})).unwrap();
		crate::routes::admin::models::patch_model(
			State(std::sync::Arc::clone(state)),
			crate::types::AuthUser(user.clone()),
			Path(model_id),
			axum::Json(body),
		)
		.await
		.into_response()
	}

	async fn stored_icon(pool: &PgPool, model_id: Uuid) -> Option<String> {
		sqlx::query_scalar("SELECT icon FROM model_configs WHERE model_id = $1 AND owner_id IS NULL")
			.bind(model_id)
			.fetch_optional(pool)
			.await
			.unwrap()
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn patch_model_icon_validation_rejects_xss_and_accepts_benign_svg(pool: PgPool) {
		use axum::http::StatusCode;
		crate::config::Config::init(&pool).await.expect("config loads");
		crate::i18n::I18n::init(&pool).await;
		let user_id = create_user(&pool, "icon-admin").await;
		sqlx::query("INSERT INTO user_roles (user_id, role_id) SELECT $1, id FROM roles WHERE name = 'admin'")
			.bind(user_id)
			.execute(&pool)
			.await
			.unwrap();
		let provider_id = create_provider(&pool, "Icon Provider", true).await;
		let model_id = create_model(&pool, provider_id, "icon-model", "Icon Model", true).await;
		let user = crate::types::User::find_by_id(&pool, &user_id).await.unwrap().unwrap();
		let state = crate::tests::gateway::support::job_state(pool.clone());

		for malicious in [
			"<svg onload=alert(1)>",
			"<svg><script>alert(1)</script></svg>",
			"<svg><a href=\"javascript:alert(1)\">x</a></svg>",
			"<img src=x onerror=alert(1)>",
		] {
			let response = patch_icon(&state, &user, model_id, malicious).await;
			assert_eq!(response.status(), StatusCode::BAD_REQUEST, "expected rejection for {malicious}");
			assert!(stored_icon(&pool, model_id).await.is_none(), "malicious icon must not be stored: {malicious}");
		}

		let benign = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path d=\"M20 6 9 17l-5-5\"/></svg>";
		let response = patch_icon(&state, &user, model_id, benign).await;
		assert_eq!(response.status(), StatusCode::OK);
		assert_eq!(stored_icon(&pool, model_id).await.as_deref(), Some(benign));

		let response = patch_icon(&state, &user, model_id, "").await;
		assert_eq!(response.status(), StatusCode::OK);
		assert_eq!(stored_icon(&pool, model_id).await.as_deref(), Some(""));
	}
}

#[cfg(test)]
mod catalog_pricing {
	use crate::ai::{catalog, catalog_provider};
	use crate::tests::support::create_user;
	use crate::types::models::{ModelPricing, PriceRates, PriceSheet, PriceSheetError, PriceTier, PriceWindow};
	use crate::types::providers::ProviderKind;
	use rust_decimal::{Decimal, prelude::FromPrimitive};
	use sqlx::PgPool;
	use std::error::Error;
	use uuid::Uuid;

	type TestResult = Result<(), Box<dyn Error>>;

	struct Served {
		provider_name: String,
		model_id: Uuid,
	}

	async fn serve(pool: &PgPool, native_model_id: &str) -> Result<Served, sqlx::Error> {
		let provider_name = format!("OpenAI {}", Uuid::new_v4());
		let provider_id: Uuid = sqlx::query_scalar("INSERT INTO providers (kind, name, base_url, is_enabled) VALUES ('OPENAI', $1, 'https://api.openai.com', true) RETURNING id")
			.bind(&provider_name)
			.fetch_one(pool)
			.await?;
		let model_id = sqlx::query_scalar(
			"INSERT INTO models (provider_id, model_id, display_name, capabilities, input_modalities, output_modalities, is_enabled) VALUES ($1, $2, $3, '[]', '[\"text\"]', '[\"text\"]', true) RETURNING id",
		)
		.bind(provider_id)
		.bind(format!("{}/{native_model_id}", provider_name.to_lowercase()))
		.bind(native_model_id)
		.fetch_one(pool)
		.await?;
		Ok(Served { provider_name, model_id })
	}

	fn rates(input: f64, output: f64) -> PriceRates {
		PriceRates {
			input: Some(input),
			output: Some(output),
			..PriceRates::default()
		}
	}

	fn manual(input: f64) -> PriceSheet {
		PriceSheet {
			rates: rates(input, input * 2.0),
			tiers: Some(Vec::new()),
			..PriceSheet::default()
		}
	}

	fn window(start: &str, end: &str) -> PriceWindow {
		PriceWindow {
			days: Vec::new(),
			start: start.to_string(),
			end: end.to_string(),
			timezone: "UTC".to_string(),
			rates: PriceRates {
				input: Some(0.5),
				..PriceRates::default()
			},
		}
	}

	async fn models_dev(model: &str) -> Result<omniference::catalog::ModelPricing, Box<dyn Error>> {
		let provider = catalog_provider("models.dev reference", &ProviderKind::Openai);
		Ok(catalog().catalog_pricing(&provider, model).await.ok_or("model has a models.dev price")?)
	}

	async fn audit_events(pool: &PgPool, model_id: &Uuid) -> Result<Vec<String>, sqlx::Error> {
		sqlx::query_scalar("SELECT event FROM audit_logs WHERE target_type = 'model' AND target_id = $1 ORDER BY xmin::text::bigint")
			.bind(model_id)
			.fetch_all(pool)
			.await
	}

	#[tokio::test]
	async fn cortecs_and_mistral_use_their_own_models_dev_prices() -> TestResult {
		let openai = catalog().catalog_pricing(&catalog_provider("OpenAI", &ProviderKind::Openai), "gpt-5.4").await.ok_or("openai gpt-5.4 price")?;
		let cortecs = catalog().catalog_pricing(&catalog_provider("Cortecs", &ProviderKind::Cortecs), "gpt-5.4").await.ok_or("cortecs gpt-5.4 price")?;
		assert_ne!(cortecs.input, openai.input);
		let mistral = catalog().catalog_pricing(&catalog_provider("Mistral", &ProviderKind::Mistral), "codestral-latest").await;
		assert!(mistral.is_some());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_model_without_a_manual_price_reports_the_models_dev_price(pool: PgPool) -> TestResult {
		let reference = models_dev("gpt-5").await?;
		let served = serve(&pool, "gpt-5").await?;

		let pricing = ModelPricing::effective(&pool, &served.model_id).await?.ok_or("model missing")?;
		assert_eq!(pricing.reported_input, Decimal::from_f64(reference.input));
		assert_eq!(pricing.reported_output, Decimal::from_f64(reference.output));
		assert!(pricing.override_pricing.is_none());
		assert_eq!(pricing.effective, pricing.catalog);
		assert_eq!(Some(pricing.effective_input), pricing.reported_input);

		let tokens = crate::types::TokenEstimate { input: 1_000_000, output: 0 };
		assert_eq!(ModelPricing::estimate_cost(&pool, &served.model_id, tokens).await?, pricing.effective_input);
		assert!(!ModelPricing::is_free(&pool, &served.model_id).await?);
		assert!(ModelPricing::priced_model_ids(&pool).await?.contains(&served.model_id));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_manual_price_reaches_omniference_for_its_provider_only(pool: PgPool) -> TestResult {
		let reference = models_dev("gpt-5").await?;
		let priced = serve(&pool, "gpt-5").await?;
		let other = serve(&pool, "gpt-5").await?;
		let actor = create_user(&pool).await?;
		let priced_provider = catalog_provider(&priced.provider_name, &ProviderKind::Openai);
		let other_provider = catalog_provider(&other.provider_name, &ProviderKind::Openai);

		let saved = ModelPricing::save_override(&pool, &priced.model_id, manual(42.0), &actor).await?;
		assert_eq!(saved.effective_input, Decimal::from(42));
		assert_eq!(saved.reported_input, Decimal::from_f64(reference.input));
		let untouched = ModelPricing::effective(&pool, &other.model_id).await?.ok_or("model missing")?;
		assert_eq!(untouched.reported_input, Decimal::from_f64(reference.input));
		assert!(untouched.override_pricing.is_none());

		let engine_price = catalog().effective_pricing(&priced_provider, "gpt-5").await.ok_or("override missing")?;
		assert_eq!((engine_price.input, engine_price.output), (42.0, 84.0));
		assert!(engine_price.tiers.is_empty());
		assert_eq!(catalog().effective_pricing(&other_provider, "gpt-5").await, Some(reference));
		let overrides = ModelPricing::engine_overrides(&pool, &catalog()).await?;
		assert_eq!(overrides.len(), 1);
		assert_eq!((overrides[0].0.as_str(), overrides[0].1.as_str()), (priced.provider_name.as_str(), "gpt-5"));

		ModelPricing::delete_override(&pool, &priced.model_id, &actor).await?;
		assert_eq!(catalog().pricing_override(&priced_provider, "gpt-5").await, None);
		assert!(ModelPricing::engine_overrides(&pool, &catalog()).await?.is_empty());
		assert_eq!(audit_events(&pool, &priced.model_id).await?, vec!["model_pricing_updated", "model_pricing_deleted"]);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_partial_override_keeps_the_catalog_rates_and_tiers_it_leaves_unset(pool: PgPool) -> TestResult {
		let reference = models_dev("gpt-5.4").await?;
		assert!(!reference.tiers.is_empty(), "gpt-5.4 has context tiers on models.dev");
		let served = serve(&pool, "gpt-5.4").await?;
		let actor = create_user(&pool).await?;
		let sheet = PriceSheet {
			rates: PriceRates {
				input: Some(1.0),
				..PriceRates::default()
			},
			windows: Some(vec![window("16:30", "00:30")]),
			..PriceSheet::default()
		};

		let saved = ModelPricing::save_override(&pool, &served.model_id, sheet, &actor).await?;
		let effective = saved.effective.ok_or("effective price missing")?;
		let summary = effective.summary.ok_or("summary missing")?;
		let effective = effective.price;
		assert_eq!(effective.rates.input, Some(1.0));
		assert_eq!(effective.rates.output, Some(reference.output));
		assert_eq!(effective.tiers.as_ref().map(Vec::len), Some(reference.tiers.len()));
		assert_eq!(summary.windows, 1);
		assert!(summary.input.min <= 0.5 && summary.output.max > reference.output);

		let engine_price = catalog()
			.effective_pricing(&catalog_provider(&served.provider_name, &ProviderKind::Openai), "gpt-5.4")
			.await
			.ok_or("override missing")?;
		assert_eq!(engine_price.tiers.len(), reference.tiers.len());
		assert_eq!(engine_price.windows.len(), 1);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn removing_an_override_falls_back_to_the_catalog_tiers(pool: PgPool) -> TestResult {
		let reference = models_dev("gpt-5.4").await?;
		let served = serve(&pool, "gpt-5.4").await?;
		let actor = create_user(&pool).await?;

		let saved = ModelPricing::save_override(&pool, &served.model_id, manual(1.0), &actor).await?;
		assert_eq!(saved.effective.and_then(|sheet| sheet.price.tiers).map(|tiers| tiers.len()), Some(0));
		ModelPricing::delete_override(&pool, &served.model_id, &actor).await?;

		let pricing = ModelPricing::effective(&pool, &served.model_id).await?.ok_or("model missing")?;
		assert_eq!(pricing.effective.and_then(|sheet| sheet.price.tiers).map(|tiers| tiers.len()), Some(reference.tiers.len()));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn budget_estimates_use_the_context_tier_the_prompt_reaches(pool: PgPool) -> TestResult {
		let reference = models_dev("gpt-5.4").await?;
		let tier = reference.tiers.iter().min_by_key(|tier| tier.min_context_tokens).ok_or("no tier")?;
		let tier_input = tier.input.ok_or("tier has no input rate")?;
		let served = serve(&pool, "gpt-5.4").await?;

		let tokens = crate::types::TokenEstimate { input: u64::from(tier.min_context_tokens), output: 0 };
		let estimate = ModelPricing::estimate_cost(&pool, &served.model_id, tokens).await?;
		let expected = Decimal::from_f64(tier_input * f64::from(tier.min_context_tokens) / 1_000_000.0).ok_or("decimal")?;
		assert_eq!(estimate.round_dp(6), expected.round_dp(6));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn invalid_prices_are_refused_and_not_stored(pool: PgPool) -> TestResult {
		let known = serve(&pool, "gpt-5").await?;
		let unknown = serve(&pool, "in-house-model").await?;
		let actor = create_user(&pool).await?;
		let only_input = PriceSheet {
			rates: PriceRates {
				input: Some(1.0),
				..PriceRates::default()
			},
			..PriceSheet::default()
		};
		let negative = PriceSheet {
			rates: rates(-1.0, 1.0),
			..PriceSheet::default()
		};
		let overlapping = PriceSheet {
			windows: Some(vec![window("22:00", "02:00"), window("01:00", "03:00")]),
			..PriceSheet::default()
		};
		let zero_tier = PriceSheet {
			tiers: Some(vec![PriceTier {
				min_context_tokens: 0,
				rates: rates(1.0, 1.0),
			}]),
			..PriceSheet::default()
		};

		let refused = |result: Result<ModelPricing, PriceSheetError>| result.err().map(|error| error.reason());
		assert_eq!(refused(ModelPricing::save_override(&pool, &unknown.model_id, only_input, &actor).await), Some("missing_base_rates"));
		assert_eq!(refused(ModelPricing::save_override(&pool, &known.model_id, negative, &actor).await), Some("invalid_rate"));
		assert_eq!(refused(ModelPricing::save_override(&pool, &known.model_id, overlapping, &actor).await), Some("overlapping_windows"));
		assert_eq!(refused(ModelPricing::save_override(&pool, &known.model_id, zero_tier, &actor).await), Some("zero_threshold"));
		assert!(ModelPricing::engine_overrides(&pool, &catalog()).await?.is_empty());
		assert!(audit_events(&pool, &known.model_id).await?.is_empty());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_model_the_catalog_does_not_know_has_no_price_and_counts_as_priced(pool: PgPool) -> TestResult {
		let served = serve(&pool, "in-house-model").await?;
		let actor = create_user(&pool).await?;

		let pricing = ModelPricing::effective(&pool, &served.model_id).await?.ok_or("model missing")?;
		assert_eq!((pricing.reported_input, pricing.reported_output), (None, None));
		assert_eq!(pricing.effective_input, Decimal::ZERO);
		let tokens = crate::types::TokenEstimate { input: 1_000, output: 1_000 };
		assert_eq!(ModelPricing::estimate_cost(&pool, &served.model_id, tokens).await?, Decimal::ZERO);
		assert!(!ModelPricing::is_free(&pool, &served.model_id).await?);
		assert!(ModelPricing::priced_model_ids(&pool).await?.contains(&served.model_id));

		ModelPricing::save_override(&pool, &served.model_id, manual(0.0), &actor).await?;
		assert!(ModelPricing::is_free(&pool, &served.model_id).await?);
		assert!(!ModelPricing::priced_model_ids(&pool).await?.contains(&served.model_id));
		Ok(())
	}
}

#[cfg(test)]
mod price_sheets {
	use crate::types::models::{PriceRates, PriceServiceTier, PriceSheet, PriceTier, PriceWeekday, PriceWindow};
	use std::collections::BTreeMap;
	use std::error::Error;

	type TestResult = Result<(), Box<dyn Error>>;

	fn rates(input: f64, output: f64) -> PriceRates {
		PriceRates {
			input: Some(input),
			output: Some(output),
			..PriceRates::default()
		}
	}

	fn window(days: Vec<PriceWeekday>, start: &str, end: &str, timezone: &str) -> PriceWindow {
		PriceWindow {
			days,
			start: start.to_string(),
			end: end.to_string(),
			timezone: timezone.to_string(),
			rates: PriceRates::default(),
		}
	}

	fn catalog() -> PriceSheet {
		PriceSheet {
			rates: PriceRates {
				cache_read: Some(0.25),
				..rates(2.0, 8.0)
			},
			tiers: Some(vec![PriceTier {
				min_context_tokens: 200_001,
				rates: rates(4.0, 12.0),
			}]),
			..PriceSheet::default()
		}
	}

	fn with_windows(windows: Vec<PriceWindow>) -> PriceSheet {
		PriceSheet {
			windows: Some(windows),
			..PriceSheet::default()
		}
	}

	#[test]
	fn an_override_fills_only_what_it_sets() {
		let sheet = PriceSheet {
			rates: PriceRates {
				output: Some(6.0),
				..PriceRates::default()
			},
			..PriceSheet::default()
		};
		let merged = sheet.over(Some(&catalog()));
		assert_eq!(merged.rates.input, Some(2.0));
		assert_eq!(merged.rates.output, Some(6.0));
		assert_eq!(merged.rates.cache_read, Some(0.25));
		assert_eq!(merged.tiers, catalog().tiers);

		let no_tiers = PriceSheet { tiers: Some(Vec::new()), ..sheet }.over(Some(&catalog()));
		assert_eq!(no_tiers.tiers, Some(Vec::new()));
	}

	#[test]
	fn a_sheet_round_trips_through_the_engine_price() -> TestResult {
		let mut sheet = catalog();
		sheet.service_tiers = Some(BTreeMap::from([("flex".to_string(), PriceServiceTier { rates: rates(1.0, 4.0), tiers: Vec::new() })]));
		sheet.windows = Some(vec![window(vec![PriceWeekday::Mon], "16:30", "00:30", "Asia/Shanghai")]);
		let engine = sheet.to_engine().ok_or("priced sheet")?;
		assert_eq!(engine.service_tiers.len(), 1);
		assert_eq!(engine.windows.len(), 1);
		assert_eq!(PriceSheet::from_engine(&engine).over(None).to_engine(), Some(engine));
		assert!(PriceSheet::default().to_engine().is_none());
		Ok(())
	}

	#[test]
	fn the_summary_spans_every_layer() -> TestResult {
		let mut sheet = catalog();
		sheet.service_tiers = Some(BTreeMap::from([("flex".to_string(), PriceServiceTier { rates: rates(1.0, 4.0), tiers: Vec::new() })]));
		let summary = sheet.summary().ok_or("summary")?;
		assert_eq!((summary.input.min, summary.input.max), (1.0, 4.0));
		assert_eq!((summary.output.min, summary.output.max), (4.0, 12.0));
		assert_eq!(summary.context_tiers, vec![200_001]);
		assert_eq!(summary.service_tiers, vec!["flex".to_string()]);
		assert_eq!(summary.windows, 0);
		Ok(())
	}

	#[test]
	fn normalizing_sorts_tiers_and_lowercases_service_tiers() -> TestResult {
		let sheet = PriceSheet {
			tiers: Some(vec![
				PriceTier { min_context_tokens: 500, rates: PriceRates::default() },
				PriceTier { min_context_tokens: 100, rates: PriceRates::default() },
			]),
			service_tiers: Some(BTreeMap::from([(" Flex ".to_string(), PriceServiceTier::default())])),
			..PriceSheet::default()
		}
		.normalized();
		let thresholds: Vec<u32> = sheet.tiers.ok_or("tiers")?.iter().map(|tier| tier.min_context_tokens).collect();
		assert_eq!(thresholds, vec![100, 500]);
		assert!(sheet.service_tiers.ok_or("service tiers")?.contains_key("flex"));
		Ok(())
	}

	#[test]
	fn windows_are_checked_for_overlaps_per_timezone() {
		let base = Some(&catalog());
		let check = |windows| with_windows(windows).validate(base).err().map(|error| error.reason());
		assert_eq!(check(vec![window(vec![], "16:30", "00:30", "UTC"), window(vec![], "08:00", "12:00", "UTC")]), None);
		assert_eq!(check(vec![window(vec![], "16:30", "00:30", "UTC"), window(vec![], "00:00", "01:00", "UTC")]), Some("overlapping_windows"));
		assert_eq!(check(vec![window(vec![], "16:30", "00:30", "UTC"), window(vec![], "00:00", "01:00", "Europe/Berlin")]), None);
		assert_eq!(
			check(vec![window(vec![PriceWeekday::Sun], "23:00", "02:00", "UTC"), window(vec![PriceWeekday::Mon], "01:00", "03:00", "UTC")]),
			Some("overlapping_windows")
		);
		assert_eq!(
			check(vec![window(vec![PriceWeekday::Sat], "23:00", "02:00", "UTC"), window(vec![PriceWeekday::Mon], "01:00", "03:00", "UTC")]),
			None
		);
		assert_eq!(check(vec![window(vec![], "09:00", "09:00", "UTC")]), Some("empty_window"));
		assert_eq!(check(vec![window(vec![], "25:00", "09:00", "UTC")]), Some("invalid_time"));
		assert_eq!(check(vec![window(vec![], "08:00", "09:00", "Mars/Olympus")]), Some("invalid_timezone"));
	}

	#[test]
	fn service_tier_names_are_checked() {
		let sheet = |name: &str| PriceSheet {
			service_tiers: Some(BTreeMap::from([(name.to_string(), PriceServiceTier::default())])),
			..PriceSheet::default()
		};
		assert!(sheet("priority").validate(Some(&catalog())).is_ok());
		assert_eq!(sheet("").validate(Some(&catalog())).err().map(|error| error.reason()), Some("invalid_service_tier"));
		assert_eq!(sheet("has space").validate(Some(&catalog())).err().map(|error| error.reason()), Some("invalid_service_tier"));
	}
}
