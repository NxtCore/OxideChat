use sqlx::PgPool;

use super::{OAuthProvider, OAuthUserInfo};
use crate::types::User;
use crate::types::error::is_unique_violation;
use crate::utils::auth::{MAX_USERNAME_LENGTH, MIN_USERNAME_LENGTH};

// ============================================================================
// Internal Types
// ============================================================================

/// How a sign-in ended up at an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInKind {
	Existing,
	Linked,
	Created,
}

/// Why a sign-in was refused.
#[derive(Debug)]
pub enum SignInError {
	MissingEmail,
	EmailNotVerified,
	RegistrationClosed,
	Database(sqlx::Error),
}

impl From<sqlx::Error> for SignInError {
	fn from(error: sqlx::Error) -> Self {
		Self::Database(error)
	}
}

const MAX_BASE_USERNAME_LENGTH: usize = MAX_USERNAME_LENGTH - 6;

fn username_for(info: &OAuthUserInfo) -> String {
	let allowed = |c: &char| c.is_alphanumeric() || *c == '_' || *c == '-';
	let name: String = info.username.iter().flat_map(|name| name.chars()).filter(allowed).take(MAX_BASE_USERNAME_LENGTH).collect();
	if name.chars().count() >= MIN_USERNAME_LENGTH {
		return name;
	}
	let id: String = info.provider_user_id.chars().filter(char::is_ascii_alphanumeric).take(8).collect();
	format!("user_{id}")
}

impl OAuthProvider {
	/// Finds the account for a provider login, links it by verified email, or creates it.
	///
	/// The email only counts when the provider says it is verified, or when an admin trusts the provider's emails
	/// and it does not say otherwise. Emails are compared without regard to case. When a concurrent sign-in with the
	/// same identity wins the race, its account is used.
	///
	/// # Errors
	///
	/// Returns `SignInError` when the email is missing or unverified, when this provider may not create accounts, or on a database failure.
	pub async fn sign_in(&self, pool: &PgPool, info: &OAuthUserInfo) -> Result<(User, SignInKind), SignInError> {
		if let Some(user) = User::find_by_identity(pool, &self.slug, &info.provider_user_id).await? {
			return Ok((user, SignInKind::Existing));
		}
		match self.link_or_create(pool, info).await {
			Err(SignInError::Database(error)) if is_unique_violation(&error) => User::find_by_identity(pool, &self.slug, &info.provider_user_id)
				.await?
				.map(|user| (user, SignInKind::Existing))
				.ok_or(SignInError::Database(error)),
			result => result,
		}
	}

	async fn link_or_create(&self, pool: &PgPool, info: &OAuthUserInfo) -> Result<(User, SignInKind), SignInError> {
		let email = info.email.as_deref().ok_or(SignInError::MissingEmail)?;
		if !info.email_verified.unwrap_or(self.trust_email) {
			return Err(SignInError::EmailNotVerified);
		}
		if let Some(user) = User::find_by_email(pool, email).await? {
			user.link_identity(pool, info).await?;
			return Ok((user, SignInKind::Linked));
		}
		if !self.allow_registration {
			return Err(SignInError::RegistrationClosed);
		}
		let username = User::unique_username(pool, &username_for(info)).await?;
		let user = User::create_from_oauth(pool, email, &username, info).await?;
		user.initialize_defaults(pool).await?;
		Ok((user, SignInKind::Created))
	}
}
