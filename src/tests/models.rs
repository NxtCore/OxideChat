#[cfg(test)]
mod tests {
	use crate::types::models::{Model, ModelViewer};
	use crate::types::models_configs::ModelConfig;
	use sqlx::PgPool;
	use sqlx::types::Json;
	use uuid::Uuid;

	async fn create_user(pool: &PgPool, suffix: &str) -> Uuid {
		sqlx::query_scalar::<_, Uuid>(
			r#"
			INSERT INTO users (email, username, password_hash)
			VALUES ($1, $2, 'hash')
			RETURNING id
			"#,
		)
		.bind(format!("{suffix}@example.com"))
		.bind(format!("user_{suffix}"))
		.fetch_one(pool)
		.await
		.unwrap()
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
		let response = Model::list_for_user(&pool, viewer, 1, 10, false, None, false, None).await.unwrap();
		assert_eq!(response.items.len(), 1);
		assert_eq!(response.items[0].capabilities, vec!["user"]);
		assert_eq!(response.items[0].icon.as_deref(), Some("user-icon"));
		assert!(response.items[0].is_favorite);

		let other_viewer = ModelViewer { user_id: &other_user_id };
		let response = Model::list_for_user(&pool, other_viewer, 1, 10, false, None, false, None).await.unwrap();
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
		let hidden = Model::list_for_user(&pool, viewer, 1, 10, false, None, false, None).await.unwrap();
		assert!(hidden.items.is_empty());

		let viewer = ModelViewer { user_id: &user_id };
		let visible = Model::list_for_user(&pool, viewer, 1, 10, true, None, false, None).await.unwrap();
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

		let percent = Model::list_for_admin(&pool, 1, 10, Some("100%".to_string())).await.unwrap();
		assert_eq!(percent.items.len(), 1);
		assert_eq!(percent.items[0].display_name, "100% Real");

		let underscore = Model::list_for_admin(&pool, 1, 10, Some("under_".to_string())).await.unwrap();
		assert_eq!(underscore.items.len(), 1);
		assert_eq!(underscore.items[0].display_name, "under_score");

		let slash = Model::list_for_admin(&pool, 1, 10, Some(r"slash\".to_string())).await.unwrap();
		assert_eq!(slash.items.len(), 1);
		assert_eq!(slash.items[0].display_name, r"slash\name");
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn pagination_uses_extra_row_for_has_more(pool: PgPool) {
		let provider_id = create_provider(&pool, "Pagination Provider", true).await;
		create_model(&pool, provider_id, "page-a", "Page A", true).await;
		create_model(&pool, provider_id, "page-b", "Page B", true).await;
		create_model(&pool, provider_id, "page-c", "Page C", true).await;

		let first = Model::list_for_admin(&pool, 1, 2, None).await.unwrap();
		assert_eq!(first.items.len(), 2);
		assert!(first.has_more);

		let second = Model::list_for_admin(&pool, 2, 2, None).await.unwrap();
		assert_eq!(second.items.len(), 1);
		assert!(!second.has_more);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn patch_name_can_sync_model_config_name(pool: PgPool) {
		let provider_id = create_provider(&pool, "Patch Provider", true).await;
		let model_id = create_model(&pool, provider_id, "patch-model", "Old Name", true).await;
		let mut tx = pool.begin().await.unwrap();

		Model::patch(&mut *tx, &model_id, Some("New Name"), None).await.unwrap();
		let (display_name, stable_key) = Model::find_name_and_model_id(&mut *tx, &model_id).await.unwrap().unwrap();
		ModelConfig::ensure_system_config(&mut *tx, &model_id, &stable_key, &display_name).await.unwrap();
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

		let by_display = Model::list_for_user(&pool, viewer, 1, 10, false, Some("GPT"), false, None).await.unwrap();
		assert_eq!(by_display.items.len(), 2);

		let viewer = ModelViewer { user_id: &user_id };
		let by_model_id = Model::list_for_user(&pool, viewer, 1, 10, false, Some("claude"), false, None).await.unwrap();
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
		let favorites = Model::list_for_user(&pool, viewer, 1, 10, false, None, true, None).await.unwrap();
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
		let filtered = Model::list_for_user(&pool, viewer, 1, 10, false, None, false, Some(&provider_a)).await.unwrap();
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
}
