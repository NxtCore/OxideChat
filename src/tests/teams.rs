#[cfg(test)]
mod tests {
	use crate::types::teams::{CreateTeamRequest, Team};
	use sqlx::PgPool;
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
		.bind(format!("team_user_{suffix}"))
		.fetch_one(pool)
		.await
		.unwrap()
	}

	async fn create_provider(pool: &PgPool, name: &str) -> Uuid {
		sqlx::query_scalar::<_, Uuid>(
			r#"
			INSERT INTO providers (kind, name, base_url, is_enabled)
			VALUES ('OPENAI', $1, 'https://example.com/v1', true)
			RETURNING id
			"#,
		)
		.bind(name)
		.fetch_one(pool)
		.await
		.unwrap()
	}

	async fn create_model(pool: &PgPool, provider_id: Uuid, model_id: &str) -> Uuid {
		sqlx::query_scalar::<_, Uuid>(
			r#"
			INSERT INTO models (provider_id, model_id, display_name, is_enabled)
			VALUES ($1, $2, $2, true)
			RETURNING id
			"#,
		)
		.bind(provider_id)
		.bind(model_id)
		.fetch_one(pool)
		.await
		.unwrap()
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn default_team_exists_and_user_defaults_join_it(pool: PgPool) {
		let default_id = Team::default_id(&pool).await.unwrap();
		let user_id = create_user(&pool, "default").await;
		Team::ensure_default_membership(&pool, &user_id).await.unwrap();

		let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_members WHERE team_id = $1 AND user_id = $2")
			.bind(default_id)
			.bind(user_id)
			.fetch_one(&pool)
			.await
			.unwrap();
		assert_eq!(count, 1);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_member_replacement_updates_members(pool: PgPool) {
		let user_a = create_user(&pool, "member-a").await;
		let user_b = create_user(&pool, "member-b").await;
		let team = Team::create(
			&pool,
			&CreateTeamRequest {
				name: "Members".to_string(),
				description: None,
				allow_all_models: None,
				member_ids: Some(vec![user_a]),
				provider_ids: None,
				model_ids: None,
			},
		)
		.await
		.unwrap();

		team.set_members(&pool, &[user_b]).await.unwrap();
		let members = team.members(&pool).await.unwrap();
		assert_eq!(members.len(), 1);
		assert_eq!(members[0].id, user_b);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_model_replacement_updates_grants(pool: PgPool) {
		let provider_a = create_provider(&pool, "Provider A").await;
		let provider_b = create_provider(&pool, "Provider B").await;
		let model_a = create_model(&pool, provider_a, "model-a").await;
		let model_b = create_model(&pool, provider_b, "model-b").await;
		let team = Team::create(
			&pool,
			&CreateTeamRequest {
				name: "Models".to_string(),
				description: None,
				allow_all_models: None,
				member_ids: None,
				provider_ids: Some(vec![provider_a]),
				model_ids: Some(vec![model_a]),
			},
		)
		.await
		.unwrap();

		team.set_model_access(&pool, &[provider_b], &[model_b]).await.unwrap();
		let access = team.model_access(&pool).await.unwrap();
		assert_eq!(access.provider_ids, vec![provider_b]);
		assert_eq!(access.model_ids, vec![model_b]);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn default_team_cannot_be_deleted(pool: PgPool) {
		let default_id = Team::default_id(&pool).await.unwrap();
		let team = Team::find_by_id(&pool, &default_id).await.unwrap().unwrap();
		assert!(!team.delete(&pool).await.unwrap());
		assert!(Team::find_by_id(&pool, &default_id).await.unwrap().is_some());
	}
}
