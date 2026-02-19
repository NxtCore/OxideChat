//! User model and database operations.
//!
//! This module contains the User struct and its implementation, providing
//! an OOP-style interface for user-related database operations.

use sqlx::FromRow;
use uuid::Uuid;

use crate::types::auth::{PermissionNameRow, RoleNameRow};
use crate::types::{CountRow, PaginatedUsersResponse, PreferencesResponse, UserPreferences, UserResponse};

/// User database row.
#[derive(Debug, FromRow)]
pub struct User {
	pub id: Uuid,
	pub email: String,
	pub username: String,
	pub password_hash: Option<String>,
	pub auth_method: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl User {
	/// Find a user by ID.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn find_by_id(pool: &sqlx::PgPool, id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as("SELECT * FROM users WHERE id = $1").bind(id).fetch_optional(pool).await
	}

	/// Find a user by email address.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn find_by_email(pool: &sqlx::PgPool, email: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as("SELECT * FROM users WHERE email = $1").bind(email).fetch_optional(pool).await
	}

	/// Check if any users exist in the database.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn any_exist(pool: &sqlx::PgPool) -> Result<bool, sqlx::Error> {
		let row: CountRow = sqlx::query_as("SELECT COUNT(*) as count FROM users").fetch_one(pool).await?;
		Ok(row.count > 0)
	}

	/// Get all roles assigned to this user.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn roles(&self, pool: &sqlx::PgPool) -> Result<Vec<String>, sqlx::Error> {
		let roles: Vec<RoleNameRow> = sqlx::query_as(
			"SELECT r.name FROM roles r
			 INNER JOIN user_roles ur ON r.id = ur.role_id
			 WHERE ur.user_id = $1",
		)
		.bind(&self.id)
		.fetch_all(pool)
		.await?;
		Ok(roles.into_iter().map(|r| r.name).collect())
	}

	/// Get preferences for this user.
	///
	/// Returns default preferences if none exist.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn preferences(&self, pool: &sqlx::PgPool) -> Result<UserPreferences, sqlx::Error> {
		match sqlx::query_as("SELECT * FROM user_preferences WHERE user_id = $1")
			.bind(&self.id)
			.fetch_optional(pool)
			.await?
		{
			Some(prefs) => Ok(prefs),
			None => Ok(UserPreferences::default_for(self.id)),
		}
	}

	/// Get all permissions for this user (via role assignments).
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn permissions(&self, pool: &sqlx::PgPool) -> Result<Vec<String>, sqlx::Error> {
		let permissions: Vec<PermissionNameRow> = sqlx::query_as(
			"SELECT DISTINCT p.name FROM permissions p
			 INNER JOIN role_permissions rp ON p.id = rp.permission_id
			 INNER JOIN user_roles ur ON rp.role_id = ur.role_id
			 WHERE ur.user_id = $1",
		)
		.bind(&self.id)
		.fetch_all(pool)
		.await?;
		Ok(permissions.into_iter().map(|p| p.name).collect())
	}

	/// Check if this user has a specific permission.
	///
	/// Supports wildcards like `admin.*` or `*`.
	pub async fn has_permission(&self, pool: &sqlx::PgPool, permission: &str) -> bool {
		self.permissions(pool)
			.await
			.map(|perms| perms.iter().any(|p| Self::permission_matches(p, permission)))
			.unwrap_or(false)
	}

	/// Check if this user has any of the specified permissions.
	///
	/// Supports wildcards like `admin.*` or `*`.
	pub async fn has_any_permission(&self, pool: &sqlx::PgPool, permissions: &[&str]) -> bool {
		self.permissions(pool)
			.await
			.map(|user_perms| permissions.iter().any(|req| user_perms.iter().any(|p| Self::permission_matches(p, req))))
			.unwrap_or(false)
	}

	/// Convert this user to a response DTO with roles, permissions, and preferences.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if any database query fails.
	pub async fn to_response(&self, pool: &sqlx::PgPool) -> Result<UserResponse, sqlx::Error> {
		let roles = self.roles(pool).await?;
		let permissions = self.permissions(pool).await?;
		let preferences = self.preferences(pool).await?;

		Ok(UserResponse {
			id: self.id,
			email: self.email.clone(),
			username: self.username.clone(),
			auth_method: self.auth_method.clone(),
			roles,
			permissions,
			preferences: PreferencesResponse::from(preferences),
			created_at: self.created_at,
		})
	}

	/// Initialize default data for this user.
	///
	/// Creates:
	/// - Default user preferences
	/// - Default workspace named "Personal"
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if any database operation fails.
	pub async fn initialize_defaults(&self, pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
		sqlx::query(
			"INSERT INTO user_preferences (user_id, default_model_key, favorite_model_keys, streaming_animation, use_remend)
			 VALUES ($1, NULL, '[]', 'fade', true)
			 ON CONFLICT (user_id) DO NOTHING",
		)
		.bind(&self.id)
		.execute(pool)
		.await?;

		sqlx::query(
			"INSERT INTO workspaces (user_id, name, is_default, sort_order)
			 VALUES ($1, 'Personal', true, 0)
			 ON CONFLICT (user_id, name) DO NOTHING",
		)
		.bind(&self.id)
		.execute(pool)
		.await?;

		Ok(())
	}

	fn permission_matches(user_permission: &str, required: &str) -> bool {
		if user_permission == required {
			return true;
		}
		if let Some(prefix) = user_permission.strip_suffix(".*") {
			return required.starts_with(prefix) && required[prefix.len()..].starts_with('.');
		}
		user_permission == "*"
	}

	/// List users with pagination, optional search, and optional role filter.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn list_paginated(pool: &sqlx::PgPool, page: i64, per_page: i64, search: Option<&str>, role: Option<&str>) -> Result<PaginatedUsersResponse, sqlx::Error> {
		let offset = (page - 1) * per_page;
		let search_pattern = search.map(|s| format!("%{s}%"));

		let total = if let Some(role_name) = role {
			let row: CountRow = sqlx::query_as(
				r#"
				SELECT COUNT(DISTINCT u.id) as count
				FROM users u
				INNER JOIN user_roles ur ON u.id = ur.user_id
				INNER JOIN roles r ON ur.role_id = r.id
				WHERE r.name = $1
				AND ($2::text IS NULL OR u.email ILIKE $2 OR u.username ILIKE $2)
				"#,
			)
			.bind(role_name)
			.bind(&search_pattern)
			.fetch_one(pool)
			.await?;
			row.count
		} else {
			let row: CountRow = sqlx::query_as("SELECT COUNT(*) as count FROM users WHERE ($1::text IS NULL OR email ILIKE $1 OR username ILIKE $1)")
				.bind(&search_pattern)
				.fetch_one(pool)
				.await?;
			row.count
		};

		let users = if let Some(role_name) = role {
			sqlx::query_as::<_, User>(
				r#"
				SELECT DISTINCT u.*
				FROM users u
				INNER JOIN user_roles ur ON u.id = ur.user_id
				INNER JOIN roles r ON ur.role_id = r.id
				WHERE r.name = $1
				AND ($2::text IS NULL OR u.email ILIKE $2 OR u.username ILIKE $2)
				ORDER BY u.created_at DESC
				LIMIT $3 OFFSET $4
				"#,
			)
			.bind(role_name)
			.bind(&search_pattern)
			.bind(per_page)
			.bind(offset)
			.fetch_all(pool)
			.await?
		} else {
			sqlx::query_as::<_, User>(
				r#"
				SELECT * FROM users
				WHERE ($1::text IS NULL OR email ILIKE $1 OR username ILIKE $1)
				ORDER BY created_at DESC
				LIMIT $2 OFFSET $3
				"#,
			)
			.bind(&search_pattern)
			.bind(per_page)
			.bind(offset)
			.fetch_all(pool)
			.await?
		};

		let mut responses = Vec::with_capacity(users.len());
		for u in users {
			responses.push(u.to_response(pool).await?);
		}

		Ok(PaginatedUsersResponse {
			users: responses,
			total,
			page,
			per_page,
		})
	}

	/// Check if email already exists (excluding a specific user ID).
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn email_exists(pool: &sqlx::PgPool, email: &str, exclude_id: Option<&Uuid>) -> Result<bool, sqlx::Error> {
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

	/// Check if username already exists (excluding a specific user ID).
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn username_exists(pool: &sqlx::PgPool, username: &str, exclude_id: Option<&Uuid>) -> Result<bool, sqlx::Error> {
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

	/// Create a new user with email, username, and password hash.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn create(pool: &sqlx::PgPool, email: &str, username: &str, password_hash: &str) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, User>(
			r#"
			INSERT INTO users (email, username, password_hash, auth_method)
			VALUES ($1, $2, $3, 'local')
			RETURNING *
			"#,
		)
		.bind(email)
		.bind(username)
		.bind(password_hash)
		.fetch_one(pool)
		.await
	}

	/// Update email and/or username for this user.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn update(&mut self, pool: &sqlx::PgPool, email: Option<&str>, username: Option<&str>) -> Result<(), sqlx::Error> {
		let new_email = email.unwrap_or(&self.email);
		let new_username = username.unwrap_or(&self.username);

		let updated = sqlx::query_as::<_, User>("UPDATE users SET email = $2, username = $3, updated_at = NOW() WHERE id = $1 RETURNING *")
			.bind(self.id)
			.bind(new_email)
			.bind(new_username)
			.fetch_one(pool)
			.await?;

		*self = updated;
		Ok(())
	}

	/// Delete this user and all associated data.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn delete(&self, pool: &sqlx::PgPool) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM users WHERE id = $1").bind(self.id).execute(pool).await?;
		Ok(result.rows_affected() > 0)
	}

	/// Clear all roles for this user, then assign the given roles.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if any database query fails.
	pub async fn set_roles(&self, pool: &sqlx::PgPool, role_names: &[String]) -> Result<(), sqlx::Error> {
		sqlx::query("DELETE FROM user_roles WHERE user_id = $1").bind(self.id).execute(pool).await?;

		for role_name in role_names {
			sqlx::query(
				r#"
				INSERT INTO user_roles (user_id, role_id)
				SELECT $1, id FROM roles WHERE name = $2
				ON CONFLICT DO NOTHING
				"#,
			)
			.bind(self.id)
			.bind(role_name)
			.execute(pool)
			.await?;
		}

		Ok(())
	}

	/// Update the password hash for this user.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn set_password(&self, pool: &sqlx::PgPool, password_hash: &str) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1")
			.bind(self.id)
			.bind(password_hash)
			.execute(pool)
			.await?;
		Ok(())
	}

	/// Check if a role name exists in the database.
	///
	/// # Errors
	///
	/// Returns `sqlx::Error` if the database query fails.
	pub async fn role_exists(pool: &sqlx::PgPool, role_name: &str) -> Result<bool, sqlx::Error> {
		let count: CountRow = sqlx::query_as("SELECT COUNT(*) as count FROM roles WHERE name = $1")
			.bind(role_name)
			.fetch_one(pool)
			.await?;
		Ok(count.count > 0)
	}
}
