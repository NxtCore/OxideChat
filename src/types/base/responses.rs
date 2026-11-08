use crate::types::oauth::PublicOAuthProvider;
use crate::types::{Role, ThemeCssVars};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "Base"))]
pub struct BaseResponse {
	#[cfg_attr(test, ts(type = "any"))]
	pub i18n: Arc<Value>,
	/// The language for this request: the user's own choice, else the installation default.
	pub language: String,
	/// The installation's default language.
	pub default_language: String,
	pub needs_setup: bool,
	pub oauth_providers: Vec<PublicOAuthProvider>,
	pub roles: Vec<Role>,
	/// Whether the chat upstream-provider selector is enabled instance-wide.
	pub enable_provider_selector: bool,
	/// Instance-wide default model ID.
	pub default_model_id: Option<Uuid>,
	/// Theme for users who have not picked their own.
	pub default_theme: ThemeCssVars,
	/// Branding an admin set for this installation.
	pub instance: InstanceResponse,
	/// Whether chats get a generated title after their first reply.
	pub title_generation: TitleGeneration,
}

/// Admin setting for generated chat titles; the admin PATCH takes the same shape.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TitleGeneration {
	pub enabled: bool,
	/// Model that writes the title; `None` uses the chat's model.
	pub model_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "Instance"))]
pub struct InstanceResponse {
	/// Instance name; `None` means `OxideChat`.
	pub name: Option<String>,
	/// Image ID of the instance icon, served without a session at `/api/v1/images/{id}`; `None` means the default logo.
	pub icon_id: Option<Uuid>,
}
