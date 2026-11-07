use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::OAuthUserInfo;

// ============================================================================
// Request and Response Types
// ============================================================================

/// Which fields of the provider's user profile hold which value; dots reach into nested objects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ClaimMapping {
	pub subject: String,
	pub email: String,
	pub email_verified: String,
	pub name: String,
	pub avatar: String,
	pub groups: String,
}

impl Default for ClaimMapping {
	fn default() -> Self {
		Self {
			subject: "sub".to_string(),
			email: "email".to_string(),
			email_verified: "email_verified".to_string(),
			name: "name".to_string(),
			avatar: "picture".to_string(),
			groups: "groups".to_string(),
		}
	}
}

// ============================================================================
// Internal Types
// ============================================================================

fn claim<'a>(profile: &'a Value, path: &str) -> Option<&'a Value> {
	path.split('.').try_fold(profile, |value, key| value.get(key)).filter(|value| !value.is_null())
}

fn claim_string(profile: &Value, path: &str) -> Option<String> {
	match claim(profile, path)? {
		Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
		Value::Number(number) => Some(number.to_string()),
		_ => None,
	}
}

fn claim_bool(profile: &Value, path: &str) -> Option<bool> {
	match claim(profile, path)? {
		Value::Bool(flag) => Some(*flag),
		Value::String(text) => text.parse().ok(),
		_ => None,
	}
}

impl OAuthUserInfo {
	/// Reads a provider's user profile through the claim mapping.
	///
	/// Returns `None` when the profile has no subject, since the account cannot be identified then.
	#[must_use]
	pub fn from_profile(provider: &str, mapping: &ClaimMapping, profile: &Value) -> Option<Self> {
		let provider_user_id = claim_string(profile, &mapping.subject)?;
		Some(Self {
			provider: provider.to_string(),
			raw_data: serde_json::json!({ "sub": provider_user_id }),
			provider_user_id,
			email: claim_string(profile, &mapping.email),
			email_verified: claim_bool(profile, &mapping.email_verified),
			username: claim_string(profile, &mapping.name),
		})
	}
}
