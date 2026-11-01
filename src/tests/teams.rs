#[cfg(test)]
mod tests {
	use crate::tests::support::paged;
	use crate::types::teams::{CreateTeamRequest, SaveTeamRequest, Team, TeamMemberInput, TeamRole};
	use crate::tests::support::insert_user;
	use sqlx::PgPool;
	use uuid::Uuid;

	async fn create_user(pool: &PgPool, suffix: &str) -> Uuid {
		insert_user(pool, &format!("team_user_{suffix}")).await.unwrap()
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

	fn save_request(name: &str, members: Vec<TeamMemberInput>, model_ids: Vec<Uuid>) -> SaveTeamRequest {
		SaveTeamRequest {
			name: name.to_string(),
			description: Some("Saved".to_string()),
			allow_all_models: false,
			default_model_id: None,
			members,
			provider_ids: Vec::new(),
			model_ids,
		}
	}

	async fn audit_events(pool: &PgPool, team_id: Uuid) -> Vec<String> {
		sqlx::query_scalar("SELECT event FROM audit_logs WHERE target_id = $1 ORDER BY event")
			.bind(team_id)
			.fetch_all(pool)
			.await
			.unwrap()
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_save_stores_all_parts_and_audits_real_changes(pool: PgPool) {
		let actor = create_user(&pool, "save-actor").await;
		let member = create_user(&pool, "save-member").await;
		let provider = create_provider(&pool, "Save provider").await;
		let model = create_model(&pool, provider, "save-model").await;
		let team = create_team(&pool, "Before").await;

		let members = vec![TeamMemberInput { user_id: member, role: TeamRole::Admin }];
		let saved = team.save(&pool, &actor, &save_request("After", members, vec![model])).await.unwrap();

		assert_eq!(saved.name, "After");
		assert_eq!(saved.description.as_deref(), Some("Saved"));
		let stored = team.members(&pool).await.unwrap();
		assert_eq!(stored.len(), 1);
		assert_eq!(stored[0].role, "admin");
		assert_eq!(team.model_access(&pool).await.unwrap().model_ids, vec![model]);
		assert_eq!(audit_events(&pool, team.id).await, vec!["team_member_role_updated", "team_models_updated"]);

		let unchanged = vec![TeamMemberInput { user_id: member, role: TeamRole::Admin }];
		team.save(&pool, &actor, &save_request("Renamed", unchanged, vec![model])).await.unwrap();
		assert_eq!(audit_events(&pool, team.id).await.len(), 2);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_save_changes_nothing_when_a_part_fails(pool: PgPool) {
		let actor = create_user(&pool, "rollback-actor").await;
		let member = create_user(&pool, "rollback-member").await;
		let team = create_team(&pool, "Original").await;

		let members = vec![TeamMemberInput { user_id: member, role: TeamRole::Member }];
		let result = team.save(&pool, &actor, &save_request("Changed", members, vec![Uuid::new_v4()])).await;

		assert!(result.is_err());
		let stored = Team::find_by_id(&pool, &team.id).await.unwrap().unwrap();
		assert_eq!(stored.name, "Original");
		assert!(team.members(&pool).await.unwrap().is_empty());
	}

	async fn create_team(pool: &PgPool, name: &str) -> Team {
		Team::create(
			pool,
			&CreateTeamRequest {
				name: name.to_string(),
				description: None,
				allow_all_models: None,
				member_ids: None,
				provider_ids: None,
				model_ids: None,
			},
		)
		.await
		.unwrap()
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn team_search_escapes_like_wildcards(pool: PgPool) {
		create_team(&pool, "100% Real").await;
		create_team(&pool, "1000 Real").await;
		create_team(&pool, "under_score").await;
		create_team(&pool, "underXscore").await;
		create_team(&pool, r"slash\name").await;
		create_team(&pool, "slash-name").await;

		let percent = Team::list_paginated(&pool, &paged(1, 10), Some("100%")).await.unwrap();
		assert_eq!(percent.items.len(), 1);
		assert_eq!(percent.items[0].name, "100% Real");

		let underscore = Team::list_paginated(&pool, &paged(1, 10), Some("under_")).await.unwrap();
		assert_eq!(underscore.items.len(), 1);
		assert_eq!(underscore.items[0].name, "under_score");

		let slash = Team::list_paginated(&pool, &paged(1, 10), Some(r"slash\")).await.unwrap();
		assert_eq!(slash.items.len(), 1);
		assert_eq!(slash.items[0].name, r"slash\name");
	}
}
