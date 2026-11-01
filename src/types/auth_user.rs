//! Authenticated user extractor for handlers.

use crate::types::{AppError, RequestContext, User};
use crate::utils::response::ErrorCode;
use axum::{extract::FromRequestParts, http::request::Parts};
use sqlx::PgPool;
use std::ops::Deref;

// Internal Types

/// The signed-in user; extraction fails with `NotAuthenticated` when there is none.
#[derive(Debug, Clone)]
pub struct AuthUser(pub User);

impl AuthUser {
	/// Checks a permission against the database.
	///
	/// # Errors
	///
	/// Returns `InsufficientPermissions` when the permission is missing and a server error when the lookup fails.
	pub async fn require(&self, pool: &PgPool, permission: &str) -> Result<(), AppError> {
		if self.0.check_permission(pool, permission).await? {
			Ok(())
		} else {
			Err(ErrorCode::InsufficientPermissions.into())
		}
	}
}

impl Deref for AuthUser {
	type Target = User;

	fn deref(&self) -> &User {
		&self.0
	}
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
	type Rejection = AppError;

	async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, AppError> {
		parts
			.extensions
			.get::<RequestContext>()
			.and_then(|context| context.user.clone())
			.map(AuthUser)
			.ok_or_else(|| ErrorCode::NotAuthenticated.into())
	}
}
