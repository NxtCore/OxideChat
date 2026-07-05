use super::rows::EffectiveBudgetRow;
use super::{Budget, BudgetAssignmentInfo, BudgetAssignmentRequest, BudgetResponse, CreateBudgetRequest, EffectiveBudget, EffectiveBudgetResponse, UpdateBudgetRequest, UserBudgetStatus};
use crate::types::BaseType;
use crate::types::axum::PaginatedResponse;
use crate::types::models::ModelPricing;
use chrono::{DateTime, Datelike, Duration, NaiveTime, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

impl Budget {
	fn search_pattern(search: Option<&str>) -> Option<String> {
		search.map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("%{s}%"))
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
			WHERE ($1::text IS NULL OR name ILIKE $1)
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
		sqlx::query("DELETE FROM budget_assignments WHERE user_id = $1")
			.bind(user_id)
			.execute(&mut *tx)
			.await?;
		sqlx::query("INSERT INTO budget_assignments (budget_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
			.bind(self.id)
			.bind(user_id)
			.execute(&mut *tx)
			.await?;
		tx.commit().await
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

	pub async fn delete_assignment(pool: &PgPool, assignment_id: &Uuid) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM budget_assignments WHERE id = $1")
			.bind(assignment_id)
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

	pub async fn status_for_user(pool: &PgPool, user_id: &Uuid) -> Result<UserBudgetStatus, sqlx::Error> {
		let effective = Self::budgets_for_user(pool, user_id).await?;
		let mut budgets = Vec::with_capacity(effective.len());
		let mut should_block = false;
		let mut should_warn = false;
		for budget in effective {
			let (window_start, resets_at) = Self::window(&budget);
			let spent = Self::spent_in_window(pool, user_id, &budget, window_start).await?;
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

	async fn spent_in_window(pool: &PgPool, user_id: &Uuid, budget: &EffectiveBudget, window_start: DateTime<Utc>) -> Result<Decimal, sqlx::Error> {
		if budget.budget.kind == "pooled" {
			if let Some(team_id) = budget.team_id {
				return sqlx::query_scalar(
					r#"
					SELECT COALESCE(SUM(ue.cost_total), 0)
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
				.await;
			}
		}
		sqlx::query_scalar(
			r#"
			SELECT COALESCE(SUM(cost_total), 0)
			FROM usage_events
			WHERE user_id = $1 AND created_at >= $2
			"#,
		)
		.bind(user_id)
		.bind(window_start)
		.fetch_one(pool)
		.await
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
