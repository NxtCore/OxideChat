use crate::types::user::rows::{CountRow, PermissionNameRow, RoleNameRow, UserRoleRow};
use crate::types::user::{User, UserListResponse, UserResponse};
use crate::types::{BaseType, Page, PageQuery, PreferencesResponse, Team, UserPreferences};
use sqlx::PgPool;
use uuid::Uuid;

impl User {
	fn escape_like_pattern(s: &str) -> String {
		s.replace('\\', r"\\").replace('%', r"\%").replace('_', r"\_")
	}

	pub async fn find_by_id(pool: &PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as("SELECT * FROM users WHERE id = $1").bind(id).fetch_optional(pool).await
	}

	pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as("SELECT * FROM users WHERE email = $1").bind(email).fetch_optional(pool).await
	}

	pub async fn find_by_session(pool: &PgPool, session_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as(
			"SELECT u.* FROM users u
             INNER JOIN sessions s ON u.id = s.user_id
             WHERE s.id = $1 AND s.expires_at > NOW()",
		)
		.bind(session_id)
		.fetch_optional(pool)
		.await
	}

	pub async fn any_exist(pool: &PgPool) -> Result<bool, sqlx::Error> {
		let row: CountRow = sqlx::query_as("SELECT COUNT(*) as count FROM users").fetch_one(pool).await?;
		Ok(row.count > 0)
	}

	pub async fn email_exists(pool: &PgPool, email: &str, exclude_id: Option<&Uuid>) -> Result<bool, sqlx::Error> {
		let count: CountRow = if let Some(exclude) = exclude_id {
			sqlx::query_as("SELECT COUNT(*) as count FROM users WHERE email = $1 AND id != $2")
				.bind(email)
				.bind(exclude)
				.fetch_one(pool)
				.await?
		} else {
			sqlx::query_as("SELECT COUNT(*) as count FROM users WHERE email = $1")
				.bind(email)
				.fetch_one(pool)
				.await?
		};
		Ok(count.count > 0)
	}

	pub async fn username_exists(pool: &PgPool, username: &str, exclude_id: Option<&Uuid>) -> Result<bool, sqlx::Error> {
		let count: CountRow = if let Some(exclude) = exclude_id {
			sqlx::query_as("SELECT COUNT(*) as count FROM users WHERE username = $1 AND id != $2")
				.bind(username)
				.bind(exclude)
				.fetch_one(pool)
				.await?
		} else {
			sqlx::query_as("SELECT COUNT(*) as count FROM users WHERE username = $1")
				.bind(username)
				.fetch_one(pool)
				.await?
		};
		Ok(count.count > 0)
	}

	pub async fn create(pool: &PgPool, email: &str, username: &str, password_hash: &str) -> Result<Self, sqlx::Error> {
		sqlx::query_as(
			"INSERT INTO users (email, username, password_hash, auth_method)
             VALUES ($1, $2, $3, 'local')
             RETURNING *",
		)
		.bind(email)
		.bind(username)
		.bind(password_hash)
		.fetch_one(pool)
		.await
	}

	pub async fn initialize_defaults(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
		sqlx::query(
			"INSERT INTO user_preferences (user_id, default_model_key, favorite_model_keys, streaming_animation, use_remend)
             VALUES ($1, NULL, '[]', 'fade', true)
             ON CONFLICT (user_id) DO NOTHING",
		)
		.bind(self.id)
		.execute(pool)
		.await?;

		sqlx::query(
			"INSERT INTO workspaces (user_id, name, is_default, sort_order)
             VALUES ($1, 'Personal', true, 0)
             ON CONFLICT (user_id, name) DO NOTHING",
		)
		.bind(self.id)
		.execute(pool)
		.await?;

		Team::ensure_default_membership(pool, &self.id).await?;

		Ok(())
	}

	pub async fn roles(&self, pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
		let rows: Vec<RoleNameRow> = sqlx::query_as(
			"SELECT r.name FROM roles r
             INNER JOIN user_roles ur ON r.id = ur.role_id
             WHERE ur.user_id = $1",
		)
		.bind(self.id)
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(|r| r.name).collect())
	}

	pub async fn permissions(&self, pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
		let rows: Vec<PermissionNameRow> = sqlx::query_as(
			"SELECT DISTINCT p.name FROM permissions p
             INNER JOIN role_permissions rp ON p.id = rp.permission_id
             INNER JOIN user_roles ur ON rp.role_id = ur.role_id
             WHERE ur.user_id = $1",
		)
		.bind(self.id)
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(|p| p.name).collect())
	}

	pub async fn preferences(&self, pool: &PgPool) -> Result<UserPreferences, sqlx::Error> {
		match sqlx::query_as("SELECT * FROM user_preferences WHERE user_id = $1")
			.bind(self.id)
			.fetch_optional(pool)
			.await?
		{
			Some(prefs) => Ok(prefs),
			None => Ok(UserPreferences::default_for(self.id)),
		}
	}

	pub async fn check_permission(&self, pool: &PgPool, permission: &str) -> Result<bool, sqlx::Error> {
		let permissions = self.permissions(pool).await?;
		Ok(permissions.iter().any(|p| Self::permission_matches(p, permission)))
	}

	pub async fn to_response(&self, pool: &PgPool) -> Result<UserResponse, sqlx::Error> {
		let roles = self.roles(pool).await?;
		let teams = Team::summaries_for_user(pool, &self.id).await?;
		let permissions = self.permissions(pool).await?;
		let preferences = self.preferences(pool).await?;
		Ok(UserResponse {
			id: self.id,
			email: self.email.clone(),
			username: self.username.clone(),
			auth_method: self.auth_method.clone(),
			roles,
			teams,
			permissions,
			preferences: PreferencesResponse::from(preferences),
			created_at: self.created_at,
		})
	}

	pub async fn list_paginated_light(
		pool: &PgPool,
		page: &PageQuery,
		search: Option<&str>,
		role: Option<&str>,
		team_id: Option<&Uuid>,
	) -> Result<Page<UserListResponse>, sqlx::Error> {
		let pagination = Self::pagination(page);
		let search_pattern = search.map(|s| format!("%{}%", Self::escape_like_pattern(s)));

		let users: Vec<User> = sqlx::query_as(
			r#"
			SELECT DISTINCT u.*
			FROM users u
			LEFT JOIN user_roles ur ON u.id = ur.user_id
			LEFT JOIN roles r ON ur.role_id = r.id
			LEFT JOIN team_members tm ON tm.user_id = u.id
			WHERE ($1::text IS NULL OR r.name = $1)
			  AND ($2::uuid IS NULL OR tm.team_id = $2)
			  AND ($3::text IS NULL OR u.email ILIKE $3 ESCAPE '\' OR u.username ILIKE $3 ESCAPE '\')
			ORDER BY u.created_at DESC
			LIMIT $4 OFFSET $5
			"#,
		)
		.bind(role)
		.bind(team_id)
		.bind(&search_pattern)
		.bind(pagination.limit)
		.bind(pagination.offset)
		.fetch_all(pool)
		.await?;

		let has_more = users.len() > pagination.page_size;
		let users: Vec<User> = users.into_iter().take(pagination.page_size).collect();
		if users.is_empty() {
			return Ok(Page { has_more, items: Vec::new() });
		}

		let user_ids: Vec<Uuid> = users.iter().map(|u| u.id).collect();

		let all_roles: Vec<UserRoleRow> = sqlx::query_as(
			"SELECT ur.user_id, r.name FROM roles r
             INNER JOIN user_roles ur ON r.id = ur.role_id
             WHERE ur.user_id = ANY($1)",
		)
		.bind(&user_ids)
		.fetch_all(pool)
		.await?;

		let mut user_roles_map: std::collections::HashMap<Uuid, Vec<String>> = std::collections::HashMap::new();
		for row in all_roles {
			user_roles_map.entry(row.user_id).or_default().push(row.name);
		}
		let all_teams = Team::summaries_for_users(pool, &user_ids).await?;

		let items = users
			.into_iter()
			.map(|u| {
				let roles = user_roles_map.get(&u.id).cloned().unwrap_or_default();
				let teams = all_teams.get(&u.id).cloned().unwrap_or_default();
				UserListResponse {
					id: u.id,
					email: u.email,
					username: u.username,
					auth_method: u.auth_method,
					roles,
					teams,
					created_at: u.created_at,
				}
			})
			.collect();

		Ok(Page { has_more, items })
	}

	pub(super) fn permission_matches(user_permission: &str, required: &str) -> bool {
		if user_permission == required {
			return true;
		}
		if let Some(prefix) = user_permission.strip_suffix(".*") {
			return required.starts_with(prefix) && required[prefix.len()..].starts_with('.');
		}
		user_permission == "*"
	}
}
