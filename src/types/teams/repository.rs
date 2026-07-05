use super::{
	CreateTeamRequest, Team, TeamDetailedResponse, TeamMemberResponse, TeamModelAccessResponse, TeamResponse, TeamSummaryResponse, UpdateTeamBudgetRequest,
	UpdateTeamMembersRequest, UpdateTeamModelsRequest, UpdateTeamRequest,
};
use crate::types::BaseType;
use crate::types::axum::PaginatedResponse;
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::collections::HashMap;
use uuid::Uuid;

impl Team {
	fn search_pattern(search: Option<&str>) -> Option<String> {
		search.map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("%{s}%"))
	}

	pub async fn default_id(pool: &PgPool) -> Result<Uuid, sqlx::Error> {
		sqlx::query_scalar("SELECT id FROM teams WHERE is_default = true ORDER BY created_at ASC LIMIT 1")
			.fetch_one(pool)
			.await
	}

	pub async fn ensure_default_membership(pool: &PgPool, user_id: &Uuid) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
			INSERT INTO team_members (team_id, user_id)
			SELECT id, $1 FROM teams WHERE is_default = true
			ON CONFLICT DO NOTHING
			"#,
		)
		.bind(user_id)
		.execute(pool)
		.await?;
		Ok(())
	}

	pub async fn list_paginated(pool: &PgPool, page: i64, size: i64, search: Option<&str>) -> Result<PaginatedResponse<TeamResponse>, sqlx::Error> {
		let pagination = Self::pagination(page, size);
		let search = Self::search_pattern(search);

		let rows = sqlx::query_as::<
			_,
			(
				Uuid,
				String,
				Option<String>,
				bool,
				bool,
				Option<Uuid>,
				i64,
				chrono::DateTime<chrono::Utc>,
				chrono::DateTime<chrono::Utc>,
			),
		>(
			r#"
			SELECT
				t.id,
				t.name,
				t.description,
				t.is_default,
				t.allow_all_models,
				t.budget_id,
				COUNT(tm.user_id) AS member_count,
				t.created_at,
				t.updated_at
			FROM teams t
			LEFT JOIN team_members tm ON tm.team_id = t.id
			WHERE ($1::text IS NULL OR t.name ILIKE $1)
			GROUP BY t.id
			ORDER BY t.is_default DESC, t.name ASC
			LIMIT $2 OFFSET $3
			"#,
		)
		.bind(search.as_deref())
		.bind(pagination.limit)
		.bind(pagination.offset)
		.fetch_all(pool)
		.await?;

		let has_more = rows.len() > pagination.page_size;
		let items = rows
			.into_iter()
			.take(pagination.page_size)
			.map(
				|(id, name, description, is_default, allow_all_models, budget_id, member_count, created_at, updated_at)| TeamResponse {
					id,
					name,
					description,
					is_default,
					allow_all_models,
					budget_id,
					member_count,
					created_at,
					updated_at,
				},
			)
			.collect();

		Ok(PaginatedResponse { has_more, items })
	}

	pub async fn find_by_id(pool: &PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>("SELECT * FROM teams WHERE id = $1").bind(id).fetch_optional(pool).await
	}

	pub async fn create(pool: &PgPool, req: &CreateTeamRequest) -> Result<Self, sqlx::Error> {
		let team = sqlx::query_as::<_, Self>(
			r#"
			INSERT INTO teams (name, description, allow_all_models)
			VALUES ($1, $2, $3)
			RETURNING *
			"#,
		)
		.bind(req.name.trim())
		.bind(req.description.as_deref().filter(|s| !s.trim().is_empty()))
		.bind(req.allow_all_models.unwrap_or(false))
		.fetch_one(pool)
		.await?;

		if let Some(member_ids) = &req.member_ids {
			team.set_members(pool, member_ids).await?;
		}
		if req.provider_ids.is_some() || req.model_ids.is_some() {
			team.set_model_access(pool, req.provider_ids.as_deref().unwrap_or(&[]), req.model_ids.as_deref().unwrap_or(&[]))
				.await?;
		}

		Ok(team)
	}

	pub async fn update(&self, pool: &PgPool, req: &UpdateTeamRequest) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			UPDATE teams
			SET name = COALESCE($2, name),
				description = CASE WHEN $3 THEN $4 ELSE description END,
				allow_all_models = COALESCE($5, allow_all_models),
				default_model_key = CASE WHEN $6 THEN $7 ELSE default_model_key END,
				updated_at = NOW()
			WHERE id = $1
			RETURNING *
			"#,
		)
		.bind(self.id)
		.bind(req.name.as_deref().map(str::trim).filter(|s| !s.is_empty()))
		.bind(req.description.is_some())
		.bind(req.description.as_ref().and_then(|v| v.as_deref()).filter(|s| !s.trim().is_empty()))
		.bind(req.allow_all_models)
		.bind(req.default_model_key.is_some())
		.bind(req.default_model_key.as_ref().and_then(|v| v.as_deref()))
		.fetch_one(pool)
		.await
	}

	/// Resolve the effective default model key for a user.
	///
	/// Precedence: user preference → specific team default → default team default → global default.
	pub async fn resolve_default_model_key(pool: &PgPool, user_id: &Uuid, user_default_model_key: Option<String>) -> Option<String> {
		use crate::config::Config;

		if user_default_model_key.is_some() {
			return user_default_model_key;
		}

		let team_default: Option<String> = sqlx::query_scalar(
			r#"
			SELECT t.default_model_key FROM teams t
			INNER JOIN team_members tm ON tm.team_id = t.id
			WHERE tm.user_id = $1 AND t.default_model_key IS NOT NULL
			ORDER BY t.is_default ASC LIMIT 1
			"#,
		)
		.bind(user_id)
		.fetch_optional(pool)
		.await
		.ok()
		.flatten();

		if team_default.is_some() {
			return team_default;
		}

		Config::get().default_model_key()
	}

	pub async fn update_budget(&self, pool: &PgPool, req: &UpdateTeamBudgetRequest) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			UPDATE teams
			SET budget_id = $2,
				updated_at = NOW()
			WHERE id = $1
			RETURNING *
			"#,
		)
		.bind(self.id)
		.bind(req.budget_id)
		.fetch_one(pool)
		.await
	}

	pub async fn delete(&self, pool: &PgPool) -> Result<bool, sqlx::Error> {
		if self.is_default {
			return Ok(false);
		}
		let result = sqlx::query("DELETE FROM teams WHERE id = $1 AND is_default = false")
			.bind(self.id)
			.execute(pool)
			.await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn set_members(&self, pool: &PgPool, user_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		sqlx::query("DELETE FROM team_members WHERE team_id = $1").bind(self.id).execute(&mut *tx).await?;

		if self.is_default {
			sqlx::query(
				r#"
				INSERT INTO team_members (team_id, user_id)
				SELECT $1, id FROM users
				ON CONFLICT DO NOTHING
				"#,
			)
			.bind(self.id)
			.execute(&mut *tx)
			.await?;
		} else if !user_ids.is_empty() {
			let mut builder = QueryBuilder::<Postgres>::new("INSERT INTO team_members (team_id, user_id) ");
			builder.push_values(user_ids, |mut row, user_id| {
				row.push_bind(self.id).push_bind(user_id);
			});
			builder.push(" ON CONFLICT DO NOTHING");
			builder.build().execute(&mut *tx).await?;
		}

		tx.commit().await?;
		Ok(())
	}

	pub async fn set_model_access(&self, pool: &PgPool, provider_ids: &[Uuid], model_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		sqlx::query("DELETE FROM team_model_access WHERE team_id = $1")
			.bind(self.id)
			.execute(&mut *tx)
			.await?;

		if !provider_ids.is_empty() {
			let mut builder = QueryBuilder::<Postgres>::new("INSERT INTO team_model_access (team_id, provider_id) ");
			builder.push_values(provider_ids, |mut row, provider_id| {
				row.push_bind(self.id).push_bind(provider_id);
			});
			builder.push(" ON CONFLICT DO NOTHING");
			builder.build().execute(&mut *tx).await?;
		}

		if !model_ids.is_empty() {
			let mut builder = QueryBuilder::<Postgres>::new("INSERT INTO team_model_access (team_id, model_id) ");
			builder.push_values(model_ids, |mut row, model_id| {
				row.push_bind(self.id).push_bind(model_id);
			});
			builder.push(" ON CONFLICT DO NOTHING");
			builder.build().execute(&mut *tx).await?;
		}

		tx.commit().await?;
		Ok(())
	}

	pub async fn members(&self, pool: &PgPool) -> Result<Vec<TeamMemberResponse>, sqlx::Error> {
		sqlx::query_as::<_, (Uuid, String, String)>(
			r#"
			SELECT u.id, u.email, u.username
			FROM users u
			INNER JOIN team_members tm ON tm.user_id = u.id
			WHERE tm.team_id = $1
			ORDER BY u.username ASC
			"#,
		)
		.bind(self.id)
		.fetch_all(pool)
		.await
		.map(|rows| rows.into_iter().map(|(id, email, username)| TeamMemberResponse { id, email, username }).collect())
	}

	pub async fn model_access(&self, pool: &PgPool) -> Result<TeamModelAccessResponse, sqlx::Error> {
		let rows = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>)>("SELECT provider_id, model_id FROM team_model_access WHERE team_id = $1 ORDER BY created_at ASC")
			.bind(self.id)
			.fetch_all(pool)
			.await?;

		let mut provider_ids = Vec::new();
		let mut model_ids = Vec::new();
		for (provider_id, model_id) in rows {
			if let Some(id) = provider_id {
				provider_ids.push(id);
			}
			if let Some(id) = model_id {
				model_ids.push(id);
			}
		}

		Ok(TeamModelAccessResponse { provider_ids, model_ids })
	}

	pub async fn detailed_response(&self, pool: &PgPool) -> Result<TeamDetailedResponse, sqlx::Error> {
		Ok(TeamDetailedResponse {
			id: self.id,
			name: self.name.clone(),
			description: self.description.clone(),
			is_default: self.is_default,
			allow_all_models: self.allow_all_models,
			budget_id: self.budget_id,
			default_model_key: self.default_model_key.clone(),
			members: self.members(pool).await?,
			model_access: self.model_access(pool).await?,
			created_at: self.created_at,
			updated_at: self.updated_at,
		})
	}

	pub async fn summaries_for_user(pool: &PgPool, user_id: &Uuid) -> Result<Vec<TeamSummaryResponse>, sqlx::Error> {
		sqlx::query_as::<_, Team>(
			r#"
			SELECT t.*
			FROM teams t
			INNER JOIN team_members tm ON tm.team_id = t.id
			WHERE tm.user_id = $1
			ORDER BY t.is_default DESC, t.name ASC
			"#,
		)
		.bind(user_id)
		.fetch_all(pool)
		.await
		.map(|teams| teams.into_iter().map(TeamSummaryResponse::from).collect())
	}

	pub async fn summaries_for_users(pool: &PgPool, user_ids: &[Uuid]) -> Result<HashMap<Uuid, Vec<TeamSummaryResponse>>, sqlx::Error> {
		if user_ids.is_empty() {
			return Ok(HashMap::new());
		}

		let rows = sqlx::query_as::<_, (Uuid, Uuid, String, bool)>(
			r#"
			SELECT tm.user_id, t.id, t.name, t.is_default
			FROM teams t
			INNER JOIN team_members tm ON tm.team_id = t.id
			WHERE tm.user_id = ANY($1)
			ORDER BY t.is_default DESC, t.name ASC
			"#,
		)
		.bind(user_ids)
		.fetch_all(pool)
		.await?;

		let mut map: HashMap<Uuid, Vec<TeamSummaryResponse>> = HashMap::new();
		for (user_id, id, name, is_default) in rows {
			map.entry(user_id).or_default().push(TeamSummaryResponse { id, name, is_default });
		}
		Ok(map)
	}

	pub async fn set_user_teams(pool: &PgPool, user_id: &Uuid, team_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		let default_id = Self::default_id(pool).await?;
		let mut ids = Vec::with_capacity(team_ids.len() + 1);
		ids.push(default_id);
		for id in team_ids {
			if !ids.contains(id) {
				ids.push(*id);
			}
		}

		let mut tx = pool.begin().await?;
		sqlx::query("DELETE FROM team_members WHERE user_id = $1").bind(user_id).execute(&mut *tx).await?;
		let mut builder = QueryBuilder::<Postgres>::new("INSERT INTO team_members (team_id, user_id) ");
		builder.push_values(&ids, |mut row, team_id| {
			row.push_bind(team_id).push_bind(user_id);
		});
		builder.push(" ON CONFLICT DO NOTHING");
		builder.build().execute(&mut *tx).await?;
		tx.commit().await?;
		Ok(())
	}

	pub async fn user_can_use_model(pool: &PgPool, user_id: &Uuid, model_id: &Uuid) -> Result<bool, sqlx::Error> {
		sqlx::query_scalar::<_, bool>(
			r#"
			SELECT EXISTS (
				SELECT 1
				FROM models m
				INNER JOIN providers p ON p.id = m.provider_id
				INNER JOIN team_members tm ON tm.user_id = $1
				INNER JOIN teams t ON t.id = tm.team_id
				LEFT JOIN team_model_access tma_model ON tma_model.team_id = t.id AND tma_model.model_id = m.id
				LEFT JOIN team_model_access tma_provider ON tma_provider.team_id = t.id AND tma_provider.provider_id = p.id
				WHERE m.id = $2
				  AND COALESCE(m.is_enabled, false) = true
				  AND COALESCE(p.is_enabled, false) = true
				  AND (t.allow_all_models = true OR tma_model.id IS NOT NULL OR tma_provider.id IS NOT NULL)
			)
			"#,
		)
		.bind(user_id)
		.bind(model_id)
		.fetch_one(pool)
		.await
	}
}

impl From<UpdateTeamMembersRequest> for Vec<Uuid> {
	fn from(req: UpdateTeamMembersRequest) -> Self {
		req.user_ids
	}
}

impl From<UpdateTeamModelsRequest> for (Vec<Uuid>, Vec<Uuid>) {
	fn from(req: UpdateTeamModelsRequest) -> Self {
		(req.provider_ids, req.model_ids)
	}
}
