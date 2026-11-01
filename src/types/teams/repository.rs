use super::{
	CreateTeamRequest, SaveTeamRequest, Team, TeamDetailedResponse, TeamMemberInput, TeamMemberResponse, TeamModelAccessResponse, TeamResponse, TeamSummaryResponse, UpdateTeamBudgetRequest,
	TeamRole, UpdateTeamMembersRequest, UpdateTeamModelsRequest, UpdateTeamRequest,
};
use crate::types::{Page, PageQuery};
use crate::logging::AuditLog;
use crate::types::{AUDIT_TEAM_TARGET, AuditEntry, BaseType, Budget, EVENT_TEAM_MEMBER_ROLE_UPDATED, EVENT_TEAM_MODELS_UPDATED};
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

impl Team {
	fn escape_like_pattern(s: &str) -> String {
		s.replace('\\', r"\\").replace('%', r"\%").replace('_', r"\_")
	}

	fn search_pattern(search: Option<&str>) -> Option<String> {
		search
			.map(str::trim)
			.filter(|s| !s.is_empty())
			.map(|s| format!("%{}%", Self::escape_like_pattern(s)))
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

	pub async fn list_paginated(pool: &PgPool, page: &PageQuery, search: Option<&str>) -> Result<Page<TeamResponse>, sqlx::Error> {
		let pagination = Self::pagination(page);
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
			WHERE ($1::text IS NULL OR t.name ILIKE $1 ESCAPE '\')
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

		Ok(Page { has_more, items })
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
				default_model_id = CASE WHEN $6 THEN $7 ELSE default_model_id END,
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
		.bind(req.default_model_id.is_some())
		.bind(req.default_model_id.as_ref().and_then(|v| *v))
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
			SELECT m.model_id FROM teams t
			INNER JOIN team_members tm ON tm.team_id = t.id
			INNER JOIN models m ON m.id = t.default_model_id
			WHERE tm.user_id = $1
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

		let default_model_id = Config::get().default_model_id()?;
		sqlx::query_scalar("SELECT model_id FROM models WHERE id = $1")
			.bind(default_model_id)
			.fetch_optional(pool)
			.await
			.ok()
			.flatten()
	}

	/// Changes the team's budget and audits it in one transaction.
	///
	/// # Errors
	///
	/// Returns an error when the budget cannot be stored; nothing is changed then.
	pub async fn update_budget(&self, pool: &PgPool, actor_id: &Uuid, req: &UpdateTeamBudgetRequest) -> Result<Self, sqlx::Error> {
		Budget::replace_team_budget_from_legacy(pool, actor_id, &self.id, req.budget_id).await?;
		Self::find_by_id(pool, &self.id).await?.ok_or(sqlx::Error::RowNotFound)
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

	/// Saves the team's fields, members, roles and model access in one transaction.
	///
	/// # Errors
	///
	/// Returns an error when any part cannot be stored; nothing is changed then.
	pub async fn save(&self, pool: &PgPool, actor_id: &Uuid, req: &SaveTeamRequest) -> Result<Self, sqlx::Error> {
		let mut tx = pool.begin().await?;
		let updated = sqlx::query_as::<_, Self>(
			r#"
			UPDATE teams
			SET name = $2, description = $3, allow_all_models = $4, default_model_id = $5, updated_at = NOW()
			WHERE id = $1
			RETURNING *
			"#,
		)
		.bind(self.id)
		.bind(req.name.trim())
		.bind(req.description.as_deref().filter(|s| !s.trim().is_empty()))
		.bind(req.allow_all_models)
		.bind(req.default_model_id)
		.fetch_one(&mut *tx)
		.await?;

		let user_ids: Vec<Uuid> = req.members.iter().map(|member| member.user_id).collect();
		self.replace_members(&mut tx, &user_ids).await?;
		self.apply_member_roles(&mut tx, actor_id, &req.members).await?;
		let (before_providers, before_models) = self.stored_model_access(&mut tx).await?;
		if sorted_unique(&req.provider_ids) != before_providers || sorted_unique(&req.model_ids) != before_models {
			self.apply_model_access(&mut tx, actor_id, &req.provider_ids, &req.model_ids).await?;
		}
		tx.commit().await?;
		Ok(updated)
	}

	pub async fn set_members(&self, pool: &PgPool, user_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		self.replace_members(&mut tx, user_ids).await?;
		tx.commit().await?;
		Ok(())
	}

	async fn replace_members(&self, tx: &mut Transaction<'_, Postgres>, user_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		if self.is_default {
			sqlx::query(
				r#"
				INSERT INTO team_members (team_id, user_id)
				SELECT $1, id FROM users
				ON CONFLICT DO NOTHING
				"#,
			)
			.bind(self.id)
			.execute(&mut **tx)
			.await?;
		} else {
			sqlx::query("DELETE FROM team_members WHERE team_id = $1 AND user_id <> ALL($2)")
				.bind(self.id)
				.bind(user_ids)
				.execute(&mut **tx)
				.await?;
			if !user_ids.is_empty() {
				let mut builder = QueryBuilder::<Postgres>::new("INSERT INTO team_members (team_id, user_id) ");
				builder.push_values(user_ids, |mut row, user_id| {
					row.push_bind(self.id).push_bind(user_id);
				});
				builder.push(" ON CONFLICT DO NOTHING");
				builder.build().execute(&mut **tx).await?;
			}
		}

		Ok(())
	}

	async fn apply_member_roles(&self, tx: &mut Transaction<'_, Postgres>, actor_id: &Uuid, members: &[TeamMemberInput]) -> Result<(), sqlx::Error> {
		let user_ids: Vec<Uuid> = members.iter().map(|member| member.user_id).collect();
		let current: HashMap<Uuid, String> = sqlx::query_as::<_, (Uuid, String)>("SELECT user_id, role FROM team_members WHERE team_id = $1 AND user_id = ANY($2) FOR UPDATE")
			.bind(self.id)
			.bind(&user_ids)
			.fetch_all(&mut **tx)
			.await?
			.into_iter()
			.collect();
		for member in members {
			if let Some(before) = current.get(&member.user_id).filter(|before| before.as_str() != member.role.as_str()) {
				self.write_member_role(tx, actor_id, member, before).await?;
			}
		}
		Ok(())
	}

	async fn write_member_role(&self, tx: &mut Transaction<'_, Postgres>, actor_id: &Uuid, member: &TeamMemberInput, before: &str) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE team_members SET role = $3 WHERE team_id = $1 AND user_id = $2")
			.bind(self.id)
			.bind(member.user_id)
			.bind(member.role.as_str())
			.execute(&mut **tx)
			.await?;
		let metadata = serde_json::json!({ "user_id": member.user_id, "before": before, "after": member.role.as_str() });
		let entry = AuditEntry {
			event: EVENT_TEAM_MEMBER_ROLE_UPDATED,
			actor_id,
			target_type: AUDIT_TEAM_TARGET,
			target_id: &self.id,
			metadata,
		};
		AuditLog::write(tx, entry).await
	}

	/// Tells whether the user is a member of the team.
	///
	/// # Errors
	///
	/// Returns an error when membership cannot be queried.
	pub async fn is_member<'e, E: sqlx::PgExecutor<'e>>(executor: E, team_id: &Uuid, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM team_members WHERE team_id = $1 AND user_id = $2)")
			.bind(team_id)
			.bind(user_id)
			.fetch_one(executor)
			.await
	}

	/// Tells whether the user holds the admin role in the team.
	///
	/// # Errors
	///
	/// Returns an error when membership cannot be queried.
	pub async fn is_admin<'e, E: sqlx::PgExecutor<'e>>(executor: E, team_id: &Uuid, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM team_members WHERE team_id = $1 AND user_id = $2 AND role = 'admin')")
			.bind(team_id)
			.bind(user_id)
			.fetch_one(executor)
			.await
	}

	/// Changes a member's team role and audits it in the same transaction.
	///
	/// Returns `false` when the user is not a member of the team.
	///
	/// # Errors
	///
	/// Returns an error when the role cannot be stored; nothing is changed then.
	pub async fn set_member_role(&self, pool: &PgPool, actor_id: &Uuid, user_id: &Uuid, role: TeamRole) -> Result<bool, sqlx::Error> {
		let mut tx = pool.begin().await?;
		let before: Option<String> = sqlx::query_scalar("SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2 FOR UPDATE")
			.bind(self.id)
			.bind(user_id)
			.fetch_optional(&mut *tx)
			.await?;
		let Some(before) = before else {
			return Ok(false);
		};
		self.write_member_role(&mut tx, actor_id, &TeamMemberInput { user_id: *user_id, role }, &before).await?;
		tx.commit().await?;
		Ok(true)
	}

	pub async fn set_model_access(&self, pool: &PgPool, provider_ids: &[Uuid], model_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		self.replace_model_access(&mut tx, provider_ids, model_ids).await?;
		tx.commit().await?;
		Ok(())
	}

	/// Replaces the team's model access and audits the change in the same transaction.
	///
	/// # Errors
	///
	/// Returns an error when model access cannot be stored.
	pub async fn update_model_access(&self, pool: &PgPool, actor_id: &Uuid, provider_ids: &[Uuid], model_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		self.apply_model_access(&mut tx, actor_id, provider_ids, model_ids).await?;
		tx.commit().await?;
		Ok(())
	}

	async fn stored_model_access(&self, tx: &mut Transaction<'_, Postgres>) -> Result<(Vec<Uuid>, Vec<Uuid>), sqlx::Error> {
		let provider_ids = sqlx::query_scalar("SELECT provider_id FROM team_model_access WHERE team_id = $1 AND provider_id IS NOT NULL ORDER BY provider_id")
			.bind(self.id)
			.fetch_all(&mut **tx)
			.await?;
		let model_ids = sqlx::query_scalar("SELECT model_id FROM team_model_access WHERE team_id = $1 AND model_id IS NOT NULL ORDER BY model_id")
			.bind(self.id)
			.fetch_all(&mut **tx)
			.await?;
		Ok((provider_ids, model_ids))
	}

	async fn apply_model_access(&self, tx: &mut Transaction<'_, Postgres>, actor_id: &Uuid, provider_ids: &[Uuid], model_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		let (before_providers, before_models) = self.stored_model_access(tx).await?;
		self.replace_model_access(tx, provider_ids, model_ids).await?;
		let metadata = serde_json::json!({
			"before": { "provider_ids": before_providers, "model_ids": before_models },
			"after": { "provider_ids": provider_ids, "model_ids": model_ids },
		});
		let entry = AuditEntry {
			event: EVENT_TEAM_MODELS_UPDATED,
			actor_id,
			target_type: AUDIT_TEAM_TARGET,
			target_id: &self.id,
			metadata,
		};
		AuditLog::write(tx, entry).await
	}

	async fn replace_model_access(&self, tx: &mut Transaction<'_, Postgres>, provider_ids: &[Uuid], model_ids: &[Uuid]) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM team_model_access WHERE team_id = $1")
			.bind(self.id)
			.execute(&mut **tx)
			.await?;

		if !provider_ids.is_empty() {
			let mut builder = QueryBuilder::<Postgres>::new("INSERT INTO team_model_access (team_id, provider_id) ");
			builder.push_values(provider_ids, |mut row, provider_id| {
				row.push_bind(self.id).push_bind(provider_id);
			});
			builder.push(" ON CONFLICT DO NOTHING");
			builder.build().execute(&mut **tx).await?;
		}

		if !model_ids.is_empty() {
			let mut builder = QueryBuilder::<Postgres>::new("INSERT INTO team_model_access (team_id, model_id) ");
			builder.push_values(model_ids, |mut row, model_id| {
				row.push_bind(self.id).push_bind(model_id);
			});
			builder.push(" ON CONFLICT DO NOTHING");
			builder.build().execute(&mut **tx).await?;
		}

		Ok(())
	}

	pub async fn members(&self, pool: &PgPool) -> Result<Vec<TeamMemberResponse>, sqlx::Error> {
		sqlx::query_as::<_, (Uuid, String, String, String)>(
			r#"
			SELECT u.id, u.email, u.username, tm.role
			FROM users u
			INNER JOIN team_members tm ON tm.user_id = u.id
			WHERE tm.team_id = $1
			ORDER BY u.username ASC
			"#,
		)
		.bind(self.id)
		.fetch_all(pool)
		.await
		.map(|rows| rows.into_iter().map(|(id, email, username, role)| TeamMemberResponse { id, email, username, role }).collect())
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
			default_model_id: self.default_model_id,
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
		sqlx::query("DELETE FROM team_members WHERE user_id = $1 AND team_id <> ALL($2)")
			.bind(user_id)
			.bind(&ids)
			.execute(&mut *tx)
			.await?;
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

fn sorted_unique(ids: &[Uuid]) -> Vec<Uuid> {
	let mut sorted = ids.to_vec();
	sorted.sort_unstable();
	sorted.dedup();
	sorted
}
