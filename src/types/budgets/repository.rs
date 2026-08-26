use super::rows::EffectiveBudgetRow;
use super::{
	Budget, BudgetAssignmentInfo, BudgetAssignmentRequest, BudgetResetEventResponse, BudgetResetRequest, BudgetResponse, BudgetTeamSummaryResponse, CreateBudgetRequest,
	EffectiveBudget, EffectiveBudgetResponse, TeamBudgetAssignmentOverviewResponse, TeamBudgetOverviewResponse, UpdateBudgetRequest, UserBudgetOverviewResponse,
	UserBudgetStatus,
};
use crate::types::BaseType;
use crate::types::axum::PaginatedResponse;
use crate::types::models::ModelPricing;
use chrono::{DateTime, Datelike, Duration, NaiveTime, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct BudgetSpentRow {
	assignment_id: Uuid,
	spent: Decimal,
}

#[derive(sqlx::FromRow)]
struct BudgetResetRow {
	assignment_id: Uuid,
	reset_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct BudgetUserRow {
	id: Uuid,
	email: String,
	username: String,
}

#[derive(sqlx::FromRow)]
struct TeamBudgetRow {
	team_id: Uuid,
	team_name: String,
	member_count: i64,
	assignment_id: Uuid,
	budget_id: Uuid,
	budget_name: String,
	description: Option<String>,
	amount: Decimal,
	kind: String,
	interval: String,
	reset_strategy: String,
	on_exceed: String,
	is_enabled: bool,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
	assigned_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct UserEffectiveBudgetRow {
	target_user_id: Uuid,
	id: Uuid,
	name: String,
	description: Option<String>,
	amount: Decimal,
	kind: String,
	interval: String,
	reset_strategy: String,
	on_exceed: String,
	is_enabled: bool,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
	assignment_id: Uuid,
	team_id: Option<Uuid>,
	user_id: Option<Uuid>,
	assigned_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct UserTeamRow {
	user_id: Uuid,
	id: Uuid,
	name: String,
	is_default: bool,
}

#[derive(sqlx::FromRow)]
struct TeamWindowResetRow {
	assignment_id: Uuid,
	reset_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct TeamMemberSpentRow {
	assignment_id: Uuid,
	member_user_id: Uuid,
	spent: Decimal,
}

impl Budget {
	fn escape_like_pattern(s: &str) -> String {
		s.replace('\\', r"\\").replace('%', r"\%").replace('_', r"\_")
	}

	fn search_pattern(search: Option<&str>) -> Option<String> {
		search.map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("%{}%", Self::escape_like_pattern(s)))
	}

	fn budget_from_row(row: EffectiveBudgetRow) -> EffectiveBudget {
		EffectiveBudget {
			budget: Budget {
				id: row.id,
				name: row.name,
				description: row.description,
				amount: row.amount,
				kind: row.kind,
				interval: row.interval,
				reset_strategy: row.reset_strategy,
				on_exceed: row.on_exceed,
				is_enabled: row.is_enabled,
				created_at: row.created_at,
				updated_at: row.updated_at,
			},
			assignment_id: row.assignment_id,
			team_id: row.team_id,
			user_id: row.user_id,
			assigned_at: row.assigned_at,
		}
	}

	pub async fn list(pool: &PgPool, page: i64, size: i64, search: Option<&str>) -> Result<PaginatedResponse<BudgetResponse>, sqlx::Error> {
		let pagination = Self::pagination(page, size);
		let search = Self::search_pattern(search);
		let rows = sqlx::query_as::<_, Budget>(
			r#"
			SELECT id, name, description, amount, kind::text AS kind, interval::text AS interval,
			       reset_strategy::text AS reset_strategy, on_exceed::text AS on_exceed, is_enabled, created_at, updated_at
			FROM budgets
			WHERE ($1::text IS NULL OR name ILIKE $1 ESCAPE '\')
			ORDER BY name ASC
			LIMIT $2 OFFSET $3
			"#,
		)
		.bind(search.as_deref())
		.bind(pagination.limit)
		.bind(pagination.offset)
		.fetch_all(pool)
		.await?;
		let has_more = rows.len() > pagination.page_size;
		let items = rows.into_iter().take(pagination.page_size).map(BudgetResponse::from).collect();
		Ok(PaginatedResponse { has_more, items })
	}

	pub async fn find_by_id(pool: &PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			SELECT id, name, description, amount, kind::text AS kind, interval::text AS interval,
			       reset_strategy::text AS reset_strategy, on_exceed::text AS on_exceed, is_enabled, created_at, updated_at
			FROM budgets WHERE id = $1
			"#,
		)
		.bind(id)
		.fetch_optional(pool)
		.await
	}

	pub async fn create(pool: &PgPool, req: &CreateBudgetRequest) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			INSERT INTO budgets (name, description, amount, kind, interval, reset_strategy, on_exceed, is_enabled)
			VALUES ($1, $2, $3, $4::budget_kind, $5::budget_interval, $6::budget_reset_strategy, $7::budget_on_exceed, $8)
			RETURNING id, name, description, amount, kind::text AS kind, interval::text AS interval,
			          reset_strategy::text AS reset_strategy, on_exceed::text AS on_exceed, is_enabled, created_at, updated_at
			"#,
		)
		.bind(req.name.trim())
		.bind(req.description.as_deref().filter(|s| !s.trim().is_empty()))
		.bind(req.amount)
		.bind(req.kind.as_str())
		.bind(req.interval.as_str())
		.bind(req.reset_strategy.as_str())
		.bind(req.on_exceed.as_str())
		.bind(req.is_enabled.unwrap_or(true))
		.fetch_one(pool)
		.await
	}

	pub async fn update(&self, pool: &PgPool, req: &UpdateBudgetRequest) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			UPDATE budgets
			SET name = COALESCE($2, name),
			    description = CASE WHEN $3 THEN $4 ELSE description END,
			    amount = COALESCE($5, amount),
			    kind = COALESCE($6::budget_kind, kind),
			    interval = COALESCE($7::budget_interval, interval),
			    reset_strategy = COALESCE($8::budget_reset_strategy, reset_strategy),
			    on_exceed = COALESCE($9::budget_on_exceed, on_exceed),
			    is_enabled = COALESCE($10, is_enabled),
			    updated_at = NOW()
			WHERE id = $1
			RETURNING id, name, description, amount, kind::text AS kind, interval::text AS interval,
			          reset_strategy::text AS reset_strategy, on_exceed::text AS on_exceed, is_enabled, created_at, updated_at
			"#,
		)
		.bind(self.id)
		.bind(req.name.as_deref().map(str::trim).filter(|s| !s.is_empty()))
		.bind(req.description.is_some())
		.bind(req.description.as_ref().and_then(|v| v.as_deref()).filter(|s| !s.trim().is_empty()))
		.bind(req.amount)
		.bind(req.kind.as_deref())
		.bind(req.interval.as_deref())
		.bind(req.reset_strategy.as_deref())
		.bind(req.on_exceed.as_deref())
		.bind(req.is_enabled)
		.fetch_one(pool)
		.await
	}

	pub async fn delete(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM budgets WHERE id = $1").bind(self.id).execute(pool).await?;
		Ok(())
	}

	pub async fn assign_to_team(&self, pool: &PgPool, team_id: &Uuid) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		let lock_key = Self::advisory_lock_key(self.kind.as_str(), Some(team_id), None);
		sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(lock_key).execute(&mut *tx).await?;
		sqlx::query(
			r#"
			DELETE FROM budget_assignments ba
			USING budgets b
			WHERE ba.budget_id = b.id
			  AND ba.team_id = $1
			  AND b.kind = $2::budget_kind
			"#,
		)
		.bind(team_id)
		.bind(self.kind.as_str())
		.execute(&mut *tx)
		.await?;
		sqlx::query("INSERT INTO budget_assignments (budget_id, team_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
			.bind(self.id)
			.bind(team_id)
			.execute(&mut *tx)
			.await?;
		tx.commit().await
	}

	pub async fn assign_to_user(&self, pool: &PgPool, user_id: &Uuid) -> Result<(), sqlx::Error> {
		let mut tx = pool.begin().await?;
		let lock_key = Self::advisory_lock_key(self.kind.as_str(), None, Some(user_id));
		sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(lock_key).execute(&mut *tx).await?;
		sqlx::query(
			r#"
			DELETE FROM budget_assignments ba
			USING budgets b
			WHERE ba.budget_id = b.id
			  AND ba.user_id = $1
			  AND b.kind = $2::budget_kind
			"#,
		)
		.bind(user_id)
		.bind(self.kind.as_str())
		.execute(&mut *tx)
		.await?;
		sqlx::query("INSERT INTO budget_assignments (budget_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
			.bind(self.id)
			.bind(user_id)
			.execute(&mut *tx)
			.await?;
		tx.commit().await
	}

	fn advisory_lock_key(kind: &str, team_id: Option<&Uuid>, user_id: Option<&Uuid>) -> i64 {
		use std::hash::{Hash, Hasher};
		let mut h = std::collections::hash_map::DefaultHasher::new();
		kind.hash(&mut h);
		if let Some(id) = team_id {
			id.hash(&mut h);
		}
		if let Some(id) = user_id {
			id.hash(&mut h);
		}
		h.finish() as i64
	}

	pub async fn assign(&self, pool: &PgPool, req: &BudgetAssignmentRequest) -> Result<(), sqlx::Error> {
		if let Some(team_id) = req.team_id {
			self.assign_to_team(pool, &team_id).await
		} else if let Some(user_id) = req.user_id {
			self.assign_to_user(pool, &user_id).await
		} else {
			Ok(())
		}
	}

	pub async fn unassign(pool: &PgPool, req: &BudgetAssignmentRequest) -> Result<(), sqlx::Error> {
		if req.team_id.is_none() && req.user_id.is_none() {
			return Ok(());
		}
		sqlx::query(
			r#"
			DELETE FROM budget_assignments
			WHERE (($1::uuid IS NOT NULL AND team_id = $1)
			    OR ($2::uuid IS NOT NULL AND user_id = $2))
			"#,
		)
		.bind(req.team_id)
		.bind(req.user_id)
		.execute(pool)
		.await?;
		Ok(())
	}

	pub async fn delete_assignment(pool: &PgPool, budget_id: &Uuid, assignment_id: &Uuid) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM budget_assignments WHERE id = $1 AND budget_id = $2")
			.bind(assignment_id)
			.bind(budget_id)
			.execute(pool)
			.await?;
		Ok(())
	}

	pub async fn list_assignments(pool: &PgPool, budget_id: &Uuid) -> Result<Vec<BudgetAssignmentInfo>, sqlx::Error> {
		sqlx::query_as::<_, BudgetAssignmentInfo>(
			r#"
			SELECT ba.id, ba.budget_id, ba.team_id, t.name AS team_name,
			       ba.user_id, COALESCE(u.username, u.email) AS user_label, ba.assigned_at
			FROM budget_assignments ba
			LEFT JOIN teams t ON t.id = ba.team_id
			LEFT JOIN users u ON u.id = ba.user_id
			WHERE ba.budget_id = $1
			ORDER BY ba.assigned_at ASC
			"#,
		)
		.bind(budget_id)
		.fetch_all(pool)
		.await
	}

	pub async fn budgets_for_user(pool: &PgPool, user_id: &Uuid) -> Result<Vec<EffectiveBudget>, sqlx::Error> {
		let rows = sqlx::query_as::<_, EffectiveBudgetRow>(
			r#"
			SELECT b.id, b.name, b.description, b.amount, b.kind::text AS kind, b.interval::text AS interval,
			       b.reset_strategy::text AS reset_strategy, b.on_exceed::text AS on_exceed, b.is_enabled, b.created_at, b.updated_at,
			       ba.id AS assignment_id, ba.team_id, ba.user_id, ba.assigned_at
			FROM budget_assignments ba
			JOIN budgets b ON b.id = ba.budget_id
			WHERE b.is_enabled = true
			  AND (
			      ba.user_id = $1
			      OR EXISTS (
			          SELECT 1 FROM team_members tm
			          WHERE tm.team_id = ba.team_id AND tm.user_id = $1
			      )
			  )
			ORDER BY b.name ASC
			"#,
		)
		.bind(user_id)
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(Self::budget_from_row).collect())
	}

	pub async fn primary_team_id(pool: &PgPool, user_id: &Uuid) -> Result<Option<Uuid>, sqlx::Error> {
		sqlx::query_scalar(
			r#"
			SELECT t.id
			FROM teams t
			JOIN team_members tm ON tm.team_id = t.id
			WHERE tm.user_id = $1
			ORDER BY t.is_default DESC, tm.assigned_at ASC
			LIMIT 1
			"#,
		)
		.bind(user_id)
		.fetch_optional(pool)
		.await
	}

	/// Checks whether paid inference may start within every blocking budget.
	///
	/// # Errors
	/// Returns an error when effective budget status cannot be loaded.
	pub async fn allows_inference(pool: &PgPool, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		let status = Self::status_for_user(pool, user_id).await?;
		Ok(status.budgets.iter().filter(|budget| budget.on_exceed == "block").all(|budget| !budget.exhausted))
	}

	pub async fn status_for_user(pool: &PgPool, user_id: &Uuid) -> Result<UserBudgetStatus, sqlx::Error> {
		let effective = Self::budgets_for_user(pool, user_id).await?;
		let mut window_starts = Vec::with_capacity(effective.len());
		let mut resets_at = Vec::with_capacity(effective.len());
		let reset_by_assignment = Self::latest_resets_for_user(pool, user_id, &effective).await?;
		for budget in &effective {
			let (window_start, resets) = Self::window(budget);
			let window_start = reset_by_assignment
				.get(&budget.assignment_id)
				.copied()
				.filter(|reset_at| *reset_at > window_start)
				.unwrap_or(window_start);
			window_starts.push(window_start);
			resets_at.push(resets);
		}
		let spent_by_assignment = Self::spent_by_assignment(pool, user_id, &effective, &window_starts).await?;
		let mut budgets = Vec::with_capacity(effective.len());
		let mut should_block = false;
		let mut should_warn = false;
		for (index, budget) in effective.into_iter().enumerate() {
			let window_start = window_starts[index];
			let resets_at = resets_at[index];
			let spent = spent_by_assignment.get(&budget.assignment_id).copied().unwrap_or(Decimal::ZERO);
			let remaining = (budget.budget.amount - spent).max(Decimal::ZERO);
			let exhausted = spent >= budget.budget.amount;
			should_block |= exhausted && budget.budget.on_exceed == "block";
			should_warn |= exhausted && budget.budget.on_exceed == "warn";
			budgets.push(EffectiveBudgetResponse {
				amount: budget.budget.amount,
				spent,
				remaining,
				window_start,
				resets_at,
				on_exceed: budget.budget.on_exceed.clone(),
				exhausted,
				assignment_id: budget.assignment_id,
				team_id: budget.team_id,
				user_id: budget.user_id,
				budget: BudgetResponse::from(budget.budget),
			});
		}
		let blocked_model_ids = if should_block { ModelPricing::priced_model_ids(pool).await? } else { Vec::new() };
		let decision = if should_block {
			"block"
		} else if should_warn {
			"warn"
		} else {
			"allow"
		};
		Ok(UserBudgetStatus {
			budgets,
			decision: decision.to_string(),
			blocked_model_ids,
		})
	}

	async fn latest_resets_for_user(pool: &PgPool, user_id: &Uuid, budgets: &[EffectiveBudget]) -> Result<HashMap<Uuid, DateTime<Utc>>, sqlx::Error> {
		if budgets.is_empty() {
			return Ok(HashMap::new());
		}
		let mut assignment_ids = Vec::with_capacity(budgets.len());
		let mut budget_ids = Vec::with_capacity(budgets.len());
		let mut team_ids = Vec::with_capacity(budgets.len());
		let mut kinds = Vec::with_capacity(budgets.len());
		for budget in budgets {
			assignment_ids.push(budget.assignment_id);
			budget_ids.push(budget.budget.id);
			team_ids.push(budget.team_id.unwrap_or_else(Uuid::nil));
			kinds.push(budget.budget.kind.clone());
		}
		let rows = sqlx::query_as::<_, BudgetResetRow>(
			r#"
			SELECT budget_windows.assignment_id, MAX(bre.reset_at) AS reset_at
			FROM UNNEST($1::uuid[], $2::uuid[], $3::uuid[], $4::text[]) AS budget_windows(assignment_id, budget_id, team_id, kind)
			JOIN budget_reset_events bre
			  ON (bre.assignment_id = budget_windows.assignment_id AND (bre.user_id IS NULL OR bre.user_id = $5))
			  OR bre.budget_id = budget_windows.budget_id
			  OR (bre.user_id = $5 AND (bre.kind IS NULL OR bre.kind::text = budget_windows.kind))
			  OR (bre.team_id = budget_windows.team_id AND budget_windows.team_id <> $6 AND (bre.kind IS NULL OR bre.kind::text = budget_windows.kind))
			GROUP BY budget_windows.assignment_id
			"#,
		)
		.bind(&assignment_ids)
		.bind(&budget_ids)
		.bind(&team_ids)
		.bind(&kinds)
		.bind(user_id)
		.bind(Uuid::nil())
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(|row| (row.assignment_id, row.reset_at)).collect())
	}

	async fn spent_by_assignment(
		pool: &PgPool,
		user_id: &Uuid,
		budgets: &[EffectiveBudget],
		window_starts: &[DateTime<Utc>],
	) -> Result<HashMap<Uuid, Decimal>, sqlx::Error> {
		if budgets.is_empty() {
			return Ok(HashMap::new());
		}
		let mut assignment_ids = Vec::with_capacity(budgets.len());
		let mut team_ids = Vec::with_capacity(budgets.len());
		let mut is_pooled_team = Vec::with_capacity(budgets.len());
		for budget in budgets {
			assignment_ids.push(budget.assignment_id);
			team_ids.push(budget.team_id.unwrap_or_else(Uuid::nil));
			is_pooled_team.push(budget.budget.kind == "pooled" && budget.team_id.is_some());
		}
		let rows = sqlx::query_as::<_, BudgetSpentRow>(
			r#"
			SELECT budget_windows.assignment_id, COALESCE(SUM(ue.cost_total), 0)::numeric AS spent
			FROM UNNEST($1::uuid[], $2::uuid[], $3::bool[], $4::timestamptz[]) AS budget_windows(assignment_id, team_id, is_pooled_team, window_start)
			LEFT JOIN usage_events ue
			  ON ue.created_at >= budget_windows.window_start
			 AND (
			     (budget_windows.is_pooled_team AND EXISTS (
			         SELECT 1 FROM team_members tm
			         WHERE tm.team_id = budget_windows.team_id AND tm.user_id = ue.user_id
			     ))
			     OR (NOT budget_windows.is_pooled_team AND ue.user_id = $5)
			 )
			GROUP BY budget_windows.assignment_id
			"#,
		)
		.bind(&assignment_ids)
		.bind(&team_ids)
		.bind(&is_pooled_team)
		.bind(window_starts)
		.bind(user_id)
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(|row| (row.assignment_id, row.spent)).collect())
	}

	fn window(budget: &EffectiveBudget) -> (DateTime<Utc>, Option<DateTime<Utc>>) {
		match budget.budget.reset_strategy.as_str() {
			"rolling" => Self::rolling_window(&budget.budget.interval),
			"anchored" => Self::anchored_window(budget.assigned_at, &budget.budget.interval),
			_ => Self::calendar_window(&budget.budget.interval),
		}
	}

	fn rolling_window(interval: &str) -> (DateTime<Utc>, Option<DateTime<Utc>>) {
		let now = Utc::now();
		let days = Self::interval_days(interval);
		if days == 0 {
			return (Utc.timestamp_opt(0, 0).single().unwrap_or(now), None);
		}
		(now - Duration::days(days), Some(now))
	}

	fn anchored_window(assigned_at: DateTime<Utc>, interval: &str) -> (DateTime<Utc>, Option<DateTime<Utc>>) {
		let days = Self::interval_days(interval);
		if days == 0 {
			return (assigned_at, None);
		}
		let elapsed = Utc::now().signed_duration_since(assigned_at).num_days().max(0);
		let periods = elapsed / days;
		let start = assigned_at + Duration::days(periods * days);
		(start, Some(start + Duration::days(days)))
	}

	fn calendar_window(interval: &str) -> (DateTime<Utc>, Option<DateTime<Utc>>) {
		let now = Utc::now();
		let date = now.date_naive();
		let midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap_or(NaiveTime::MIN);
		match interval {
			"daily" => {
				let start = Utc.from_utc_datetime(&date.and_time(midnight));
				(start, Some(start + Duration::days(1)))
			}
			"weekly" => {
				let start_date = date - Duration::days(i64::from(date.weekday().num_days_from_monday()));
				let start = Utc.from_utc_datetime(&start_date.and_time(midnight));
				(start, Some(start + Duration::days(7)))
			}
			"monthly" => {
				let start_date = date.with_day(1).unwrap_or(date);
				let start = Utc.from_utc_datetime(&start_date.and_time(midnight));
				let next = if start_date.month() == 12 {
					start_date.with_year(start_date.year() + 1).and_then(|d| d.with_month(1))
				} else {
					start_date.with_month(start_date.month() + 1)
				}
				.unwrap_or(start_date);
				(start, Some(Utc.from_utc_datetime(&next.and_time(midnight))))
			}
			_ => (Utc.timestamp_opt(0, 0).single().unwrap_or(now), None),
		}
	}

	fn interval_days(interval: &str) -> i64 {
		match interval {
			"daily" => 1,
			"weekly" => 7,
			"monthly" => 30,
			_ => 0,
		}
	}

	pub async fn assignments(pool: &PgPool, budget_id: &Uuid) -> Result<Vec<EffectiveBudget>, sqlx::Error> {
		let rows = sqlx::query_as::<_, EffectiveBudgetRow>(
			r#"
			SELECT b.id, b.name, b.description, b.amount, b.kind::text AS kind, b.interval::text AS interval,
			       b.reset_strategy::text AS reset_strategy, b.on_exceed::text AS on_exceed, b.is_enabled, b.created_at, b.updated_at,
			       ba.id AS assignment_id, ba.team_id, ba.user_id, ba.assigned_at
			FROM budget_assignments ba
			JOIN budgets b ON b.id = ba.budget_id
			WHERE b.id = $1
			ORDER BY ba.assigned_at DESC
			"#,
		)
		.bind(budget_id)
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(Self::budget_from_row).collect())
	}

	pub async fn user_overview(pool: &PgPool) -> Result<Vec<UserBudgetOverviewResponse>, sqlx::Error> {
		let users = sqlx::query_as::<_, BudgetUserRow>(
			r#"
			SELECT id, email, username
			FROM users
			ORDER BY username ASC, email ASC
			"#,
		)
		.fetch_all(pool)
		.await?;
		if users.is_empty() {
			return Ok(Vec::new());
		}
		let user_ids: Vec<Uuid> = users.iter().map(|u| u.id).collect();
		let mut all_budgets = Self::budgets_for_all_users(pool, &user_ids).await?;
		let mut all_teams = Self::teams_for_all_users(pool, &user_ids).await?;
		let priced_model_ids = ModelPricing::priced_model_ids(pool).await?;
		let mut rows = Vec::with_capacity(users.len());
		for user in users {
			let effective = all_budgets.remove(&user.id).unwrap_or_default();
			let teams = all_teams.remove(&user.id).unwrap_or_default();
			let mut window_starts = Vec::with_capacity(effective.len());
			let mut resets_at_vec = Vec::with_capacity(effective.len());
			let reset_by_assignment = Self::latest_resets_for_user(pool, &user.id, &effective).await?;
			for budget in &effective {
				let (window_start, resets) = Self::window(budget);
				let window_start = reset_by_assignment
					.get(&budget.assignment_id)
					.copied()
					.filter(|reset_at| *reset_at > window_start)
					.unwrap_or(window_start);
				window_starts.push(window_start);
				resets_at_vec.push(resets);
			}
			let spent_by_assignment = Self::spent_by_assignment(pool, &user.id, &effective, &window_starts).await?;
			let mut budget_responses = Vec::with_capacity(effective.len());
			let mut should_block = false;
			let mut should_warn = false;
			for (index, budget) in effective.into_iter().enumerate() {
				let window_start = window_starts[index];
				let resets_at = resets_at_vec[index];
				let spent = spent_by_assignment.get(&budget.assignment_id).copied().unwrap_or(Decimal::ZERO);
				let remaining = (budget.budget.amount - spent).max(Decimal::ZERO);
				let exhausted = spent >= budget.budget.amount;
				should_block |= exhausted && budget.budget.on_exceed == "block";
				should_warn |= exhausted && budget.budget.on_exceed == "warn";
				budget_responses.push(EffectiveBudgetResponse {
					amount: budget.budget.amount,
					spent,
					remaining,
					window_start,
					resets_at,
					on_exceed: budget.budget.on_exceed.clone(),
					exhausted,
					assignment_id: budget.assignment_id,
					team_id: budget.team_id,
					user_id: budget.user_id,
					budget: BudgetResponse::from(budget.budget),
				});
			}
			let blocked_model_ids = if should_block { priced_model_ids.clone() } else { Vec::new() };
			let decision = if should_block { "block" } else if should_warn { "warn" } else { "allow" };
			let total_spent = budget_responses.iter().map(|b| b.spent).sum();
			let total_remaining = budget_responses.iter().map(|b| b.remaining).sum();
			rows.push(UserBudgetOverviewResponse {
				user_id: user.id,
				user_label: if user.username.is_empty() { user.email } else { user.username },
				teams,
				budgets: budget_responses,
				spent: total_spent,
				remaining: total_remaining,
				decision: decision.to_string(),
				blocked_model_ids,
			});
		}
		Ok(rows)
	}

	async fn budgets_for_all_users(pool: &PgPool, user_ids: &[Uuid]) -> Result<HashMap<Uuid, Vec<EffectiveBudget>>, sqlx::Error> {
		let rows = sqlx::query_as::<_, UserEffectiveBudgetRow>(
			r#"
			SELECT u.id AS target_user_id,
			       b.id, b.name, b.description, b.amount, b.kind::text AS kind, b.interval::text AS interval,
			       b.reset_strategy::text AS reset_strategy, b.on_exceed::text AS on_exceed, b.is_enabled, b.created_at, b.updated_at,
			       ba.id AS assignment_id, ba.team_id, ba.user_id, ba.assigned_at
			FROM UNNEST($1::uuid[]) AS u(id)
			JOIN budget_assignments ba ON (
			    ba.user_id = u.id
			    OR EXISTS (
			        SELECT 1 FROM team_members tm
			        WHERE tm.team_id = ba.team_id AND tm.user_id = u.id
			    )
			)
			JOIN budgets b ON b.id = ba.budget_id
			WHERE b.is_enabled = true
			ORDER BY u.id, b.name ASC
			"#,
		)
		.bind(user_ids)
		.fetch_all(pool)
		.await?;
		let mut map: HashMap<Uuid, Vec<EffectiveBudget>> = HashMap::new();
		for row in rows {
			let budget = EffectiveBudget {
				budget: Budget {
					id: row.id,
					name: row.name,
					description: row.description,
					amount: row.amount,
					kind: row.kind,
					interval: row.interval,
					reset_strategy: row.reset_strategy,
					on_exceed: row.on_exceed,
					is_enabled: row.is_enabled,
					created_at: row.created_at,
					updated_at: row.updated_at,
				},
				assignment_id: row.assignment_id,
				team_id: row.team_id,
				user_id: row.user_id,
				assigned_at: row.assigned_at,
			};
			map.entry(row.target_user_id).or_default().push(budget);
		}
		Ok(map)
	}

	async fn teams_for_all_users(pool: &PgPool, user_ids: &[Uuid]) -> Result<HashMap<Uuid, Vec<BudgetTeamSummaryResponse>>, sqlx::Error> {
		let rows = sqlx::query_as::<_, UserTeamRow>(
			r#"
			SELECT tm.user_id, t.id, t.name, t.is_default
			FROM team_members tm
			JOIN teams t ON t.id = tm.team_id
			WHERE tm.user_id = ANY($1)
			ORDER BY tm.user_id, t.is_default DESC, t.name ASC
			"#,
		)
		.bind(user_ids)
		.fetch_all(pool)
		.await?;
		let mut map: HashMap<Uuid, Vec<BudgetTeamSummaryResponse>> = HashMap::new();
		for row in rows {
			map.entry(row.user_id).or_default().push(BudgetTeamSummaryResponse { id: row.id, name: row.name, is_default: row.is_default });
		}
		Ok(map)
	}

	async fn teams_for_user(pool: &PgPool, user_id: &Uuid) -> Result<Vec<BudgetTeamSummaryResponse>, sqlx::Error> {
		sqlx::query_as::<_, BudgetTeamSummaryResponse>(
			r#"
			SELECT t.id, t.name, t.is_default
			FROM teams t
			JOIN team_members tm ON tm.team_id = t.id
			WHERE tm.user_id = $1
			ORDER BY t.is_default DESC, t.name ASC
			"#,
		)
		.bind(user_id)
		.fetch_all(pool)
		.await
	}

	pub async fn team_overview(pool: &PgPool) -> Result<Vec<TeamBudgetOverviewResponse>, sqlx::Error> {
		let rows = sqlx::query_as::<_, TeamBudgetRow>(
			r#"
			SELECT t.id AS team_id, t.name AS team_name, COUNT(tm.user_id)::bigint AS member_count,
			       ba.id AS assignment_id, b.id AS budget_id, b.name AS budget_name, b.description, b.amount,
			       b.kind::text AS kind, b.interval::text AS interval, b.reset_strategy::text AS reset_strategy,
			       b.on_exceed::text AS on_exceed, b.is_enabled, b.created_at, b.updated_at, ba.assigned_at
			FROM teams t
			LEFT JOIN team_members tm ON tm.team_id = t.id
			JOIN budget_assignments ba ON ba.team_id = t.id
			JOIN budgets b ON b.id = ba.budget_id
			WHERE b.is_enabled = true
			GROUP BY t.id, t.name, ba.id, b.id, ba.assigned_at
			ORDER BY t.name ASC, b.name ASC
			"#,
		)
		.fetch_all(pool)
		.await?;
		if rows.is_empty() {
			return Ok(Vec::new());
		}
		let effective_budgets: Vec<EffectiveBudget> = rows
			.iter()
			.map(|row| EffectiveBudget {
				budget: Budget {
					id: row.budget_id,
					name: row.budget_name.clone(),
					description: row.description.clone(),
					amount: row.amount,
					kind: row.kind.clone(),
					interval: row.interval.clone(),
					reset_strategy: row.reset_strategy.clone(),
					on_exceed: row.on_exceed.clone(),
					is_enabled: row.is_enabled,
					created_at: row.created_at,
					updated_at: row.updated_at,
				},
				assignment_id: row.assignment_id,
				team_id: Some(row.team_id),
				user_id: None,
				assigned_at: row.assigned_at,
			})
			.collect();
		let window_resets = Self::batch_team_window_resets(pool, &effective_budgets).await?;
		let mut window_starts: Vec<DateTime<Utc>> = Vec::with_capacity(effective_budgets.len());
		let mut resets_at_vec: Vec<Option<DateTime<Utc>>> = Vec::with_capacity(effective_budgets.len());
		for budget in &effective_budgets {
			let (base_window, resets) = Self::window(budget);
			let window_start = window_resets
				.get(&budget.assignment_id)
				.and_then(|r| *r)
				.filter(|reset| *reset > base_window)
				.unwrap_or(base_window);
			window_starts.push(window_start);
			resets_at_vec.push(resets);
		}
		let member_spent = Self::batch_team_member_spent(pool, &effective_budgets, &window_starts).await?;
		let mut overview = Vec::<TeamBudgetOverviewResponse>::new();
		for (index, (row, effective)) in rows.into_iter().zip(effective_budgets.into_iter()).enumerate() {
			let window_start = window_starts[index];
			let resets_at = resets_at_vec[index];
			let per_member_spends: Vec<Decimal> = member_spent
				.get(&effective.assignment_id)
				.map(|map| map.values().copied().collect())
				.unwrap_or_default();
			let (spent, remaining, exhausted_users) = if effective.budget.kind == "per_user" {
				let per_user_amount = effective.budget.amount;
				let exhausted = per_member_spends.iter().filter(|&&s| s >= per_user_amount).count() as i64;
				let total_spent: Decimal = per_member_spends.iter().copied().sum();
				let total_remaining = (per_user_amount * Decimal::from(row.member_count) - total_spent).max(Decimal::ZERO);
				(total_spent, total_remaining, exhausted)
			} else {
				let total_spent: Decimal = per_member_spends.iter().copied().sum();
				let total_remaining = (effective.budget.amount - total_spent).max(Decimal::ZERO);
				let exhausted = if total_spent >= effective.budget.amount && row.member_count > 0 { row.member_count } else { 0 };
				(total_spent, total_remaining, exhausted)
			};
			let assignment = TeamBudgetAssignmentOverviewResponse {
				assignment_id: effective.assignment_id,
				budget: BudgetResponse::from(effective.budget),
				spent,
				remaining,
				window_start,
				resets_at,
				affected_users: row.member_count,
				exhausted_users,
			};
			if let Some(existing) = overview.iter_mut().find(|team| team.team_id == row.team_id) {
				existing.spent += assignment.spent;
				existing.remaining += assignment.remaining;
				existing.exhausted_count += assignment.exhausted_users;
				existing.budgets.push(assignment);
			} else {
				overview.push(TeamBudgetOverviewResponse {
					team_id: row.team_id,
					team_name: row.team_name,
					member_count: row.member_count,
					spent: assignment.spent,
					remaining: assignment.remaining,
					exhausted_count: assignment.exhausted_users,
					budgets: vec![assignment],
				});
			}
		}
		Ok(overview)
	}

	async fn batch_team_window_resets(pool: &PgPool, budgets: &[EffectiveBudget]) -> Result<HashMap<Uuid, Option<DateTime<Utc>>>, sqlx::Error> {
		let mut assignment_ids = Vec::with_capacity(budgets.len());
		let mut budget_ids = Vec::with_capacity(budgets.len());
		let mut team_ids = Vec::with_capacity(budgets.len());
		let mut kinds = Vec::with_capacity(budgets.len());
		for budget in budgets {
			assignment_ids.push(budget.assignment_id);
			budget_ids.push(budget.budget.id);
			team_ids.push(budget.team_id.unwrap_or_else(Uuid::nil));
			kinds.push(budget.budget.kind.clone());
		}
		let rows = sqlx::query_as::<_, TeamWindowResetRow>(
			r#"
			SELECT bw.assignment_id, MAX(bre.reset_at) AS reset_at
			FROM UNNEST($1::uuid[], $2::uuid[], $3::uuid[], $4::text[]) AS bw(assignment_id, budget_id, team_id, kind)
			LEFT JOIN budget_reset_events bre
			  ON (bre.assignment_id = bw.assignment_id AND bre.user_id IS NULL)
			  OR bre.budget_id = bw.budget_id
			  OR (bre.team_id = bw.team_id AND bw.team_id <> $5 AND (bre.kind IS NULL OR bre.kind::text = bw.kind))
			GROUP BY bw.assignment_id
			"#,
		)
		.bind(&assignment_ids)
		.bind(&budget_ids)
		.bind(&team_ids)
		.bind(&kinds)
		.bind(Uuid::nil())
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(|row| (row.assignment_id, row.reset_at)).collect())
	}

	async fn batch_team_member_spent(
		pool: &PgPool,
		budgets: &[EffectiveBudget],
		window_starts: &[DateTime<Utc>],
	) -> Result<HashMap<Uuid, HashMap<Uuid, Decimal>>, sqlx::Error> {
		if budgets.is_empty() {
			return Ok(HashMap::new());
		}
		let mut assignment_ids = Vec::with_capacity(budgets.len());
		let mut team_ids = Vec::with_capacity(budgets.len());
		for budget in budgets {
			assignment_ids.push(budget.assignment_id);
			team_ids.push(budget.team_id.unwrap_or_else(Uuid::nil));
		}
		let rows = sqlx::query_as::<_, TeamMemberSpentRow>(
			r#"
			SELECT bw.assignment_id, tm.user_id AS member_user_id, COALESCE(SUM(ue.cost_total), 0)::numeric AS spent
			FROM UNNEST($1::uuid[], $2::uuid[], $3::timestamptz[]) AS bw(assignment_id, team_id, window_start)
			JOIN team_members tm ON tm.team_id = bw.team_id
			LEFT JOIN usage_events ue ON ue.user_id = tm.user_id AND ue.created_at >= bw.window_start
			GROUP BY bw.assignment_id, tm.user_id
			"#,
		)
		.bind(&assignment_ids)
		.bind(&team_ids)
		.bind(window_starts)
		.fetch_all(pool)
		.await?;
		let mut map: HashMap<Uuid, HashMap<Uuid, Decimal>> = HashMap::new();
		for row in rows {
			map.entry(row.assignment_id).or_default().insert(row.member_user_id, row.spent);
		}
		Ok(map)
	}

	async fn team_window(pool: &PgPool, budget: &EffectiveBudget) -> Result<(DateTime<Utc>, Option<DateTime<Utc>>), sqlx::Error> {
		let (window_start, resets_at) = Self::window(budget);
		let reset_at = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
			r#"
			SELECT MAX(reset_at)
			FROM budget_reset_events
			WHERE (assignment_id = $1 AND user_id IS NULL)
			   OR budget_id = $2
			   OR (team_id = $3 AND (kind IS NULL OR kind::text = $4))
			"#,
		)
		.bind(budget.assignment_id)
		.bind(budget.budget.id)
		.bind(budget.team_id)
		.bind(budget.budget.kind.as_str())
		.fetch_one(pool)
		.await?;
		let window_start = reset_at.filter(|reset| *reset > window_start).unwrap_or(window_start);
		Ok((window_start, resets_at))
	}

	async fn team_assignment_spent(pool: &PgPool, team_id: Uuid, window_start: DateTime<Utc>) -> Result<Decimal, sqlx::Error> {
		let spent = sqlx::query_scalar::<_, Decimal>(
			r#"
			SELECT COALESCE(SUM(ue.cost_total), 0)::numeric
			FROM usage_events ue
			WHERE ue.created_at >= $1
			  AND EXISTS (
			      SELECT 1 FROM team_members tm
			      WHERE tm.team_id = $2 AND tm.user_id = ue.user_id
			  )
			"#,
		)
		.bind(window_start)
		.bind(team_id)
		.fetch_one(pool)
		.await?;
		Ok(spent)
	}

	pub async fn reset(pool: &PgPool, req: &BudgetResetRequest, created_by: &Uuid) -> Result<BudgetResetEventResponse, sqlx::Error> {
		sqlx::query_as::<_, BudgetResetEventResponse>(
			r#"
			WITH inserted AS (
			    INSERT INTO budget_reset_events (assignment_id, budget_id, team_id, user_id, kind, reason, created_by)
			    VALUES ($1, $2, $3, $4, $5::budget_kind, $6, $7)
			    RETURNING id, assignment_id, budget_id, team_id, user_id, kind::text AS kind, reason, reset_at, created_by
			)
			SELECT i.id, i.assignment_id, i.budget_id, b.name AS budget_name,
			       i.team_id, t.name AS team_name,
			       i.user_id, COALESCE(u.username, u.email) AS user_label,
			       i.kind, i.reason, i.reset_at, i.created_by,
			       COALESCE(admin_user.username, admin_user.email) AS created_by_label
			FROM inserted i
			LEFT JOIN budget_assignments ba ON ba.id = i.assignment_id
			LEFT JOIN budgets b ON b.id = COALESCE(i.budget_id, ba.budget_id)
			LEFT JOIN teams t ON t.id = i.team_id
			LEFT JOIN users u ON u.id = i.user_id
			LEFT JOIN users admin_user ON admin_user.id = i.created_by
			"#,
		)
		.bind(req.assignment_id)
		.bind(req.budget_id)
		.bind(req.team_id)
		.bind(req.user_id)
		.bind(req.kind.as_deref())
		.bind(req.reason.as_deref().filter(|reason| !reason.trim().is_empty()))
		.bind(created_by)
		.fetch_one(pool)
		.await
	}

	pub async fn reset_history(pool: &PgPool) -> Result<Vec<BudgetResetEventResponse>, sqlx::Error> {
		sqlx::query_as::<_, BudgetResetEventResponse>(
			r#"
			SELECT bre.id, bre.assignment_id, bre.budget_id, b.name AS budget_name,
			       bre.team_id, t.name AS team_name,
			       bre.user_id, COALESCE(u.username, u.email) AS user_label,
			       bre.kind::text AS kind, bre.reason, bre.reset_at, bre.created_by,
			       COALESCE(admin_user.username, admin_user.email) AS created_by_label
			FROM budget_reset_events bre
			LEFT JOIN budget_assignments ba ON ba.id = bre.assignment_id
			LEFT JOIN budgets b ON b.id = COALESCE(bre.budget_id, ba.budget_id)
			LEFT JOIN teams t ON t.id = bre.team_id
			LEFT JOIN users u ON u.id = bre.user_id
			LEFT JOIN users admin_user ON admin_user.id = bre.created_by
			ORDER BY bre.reset_at DESC
			LIMIT 100
			"#,
		)
		.fetch_all(pool)
		.await
	}

	pub async fn replace_team_budget_from_legacy(pool: &PgPool, team_id: &Uuid, budget_id: Option<Uuid>) -> Result<(), sqlx::Error> {
		let Some(budget_id) = budget_id else {
			sqlx::query("DELETE FROM budget_assignments WHERE team_id = $1").bind(team_id).execute(pool).await?;
			return Ok(());
		};
		if let Some(budget) = Self::find_by_id(pool, &budget_id).await? {
			budget.assign_to_team(pool, team_id).await?;
		}
		Ok(())
	}
}
