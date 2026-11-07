use sqlx::PgPool;

use super::{OAuthProvider, OAuthUserInfo};
use crate::types::User;

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

fn fallback_username(info: &OAuthUserInfo) -> String {
	let id: String = info.provider_user_id.chars().take(8).collect();
	format!("user_{id}")
}

impl OAuthProvider {
	/// Finds the account for a provider login, links it by verified email, or creates it.
	///
	/// The email only counts when the provider says it is verified, or when an admin trusts the provider's emails
	/// and it does not say otherwise.
	///
	/// # Errors
	///
	/// Returns `SignInError` when the email is missing or unverified, when this provider may not create accounts, or on a database failure.
	pub async fn sign_in(&self, pool: &PgPool, info: &OAuthUserInfo) -> Result<(User, SignInKind), SignInError> {
		if let Some(user) = User::find_by_identity(pool, &self.slug, &info.provider_user_id).await? {
			return Ok((user, SignInKind::Existing));
		}
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
		let wanted = info.username.clone().unwrap_or_else(|| fallback_username(info));
		let username = User::unique_username(pool, &wanted).await?;
		let user = User::create_from_oauth(pool, email, &username, info).await?;
		user.initialize_defaults(pool).await?;
		Ok((user, SignInKind::Created))
	}
}
