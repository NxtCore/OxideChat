use crate::types::oauth::PublicOAuthProvider;
use crate::types::{ChatProviderSelectorResponse, ProviderRoutingSettings, Role, ThemeCssVars};
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
	/// The installation's chat provider selector and upstream provider allowlist.
	pub provider_routing: ProviderRoutingSettings,
	/// The chat provider selector as the current user gets it, after their teams' overrides.
	pub chat_provider_selector: ChatProviderSelectorResponse,
	/// Instance-wide default model ID.
	pub default_model_id: Option<Uuid>,
	/// Theme for users who have not picked their own.
	pub default_theme: ThemeCssVars,
	/// Branding an admin set for this installation.
	pub instance: InstanceResponse,
	/// Whether chats get a generated title after their first reply.
	pub title_generation: TitleGeneration,
	/// Whether visitors may create an account with email and password.
	pub registration: Registration,
	/// The storage quota for the files each user stores.
	pub storage: StorageSettings,
	/// How attached files reach the model.
	pub attachments: AttachmentSettings,
	/// Whether knowledge bases are switched on.
	pub rag: RagStatus,
}

/// Knowledge as everyone may see it; admins change it at `/api/v1/admin/rag`.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RagStatus {
	pub enabled: bool,
}

/// Admin setting for local sign-up; the admin PATCH takes the same shape. Sign-in providers have their own `allow_registration`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Registration {
	pub enabled: bool,
}

impl Default for Registration {
	fn default() -> Self {
		Self { enabled: true }
	}
}

/// Admin setting for the files users store (images and chat attachments); the admin PATCH takes the same shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct StorageSettings {
	/// Most storage, in MiB, one user's files may take (kept originals plus extracted text); `None` means no limit.
	pub quota_mb: Option<u32>,
}

impl StorageSettings {
	/// Whether a user whose files already take `stored_bytes` may store another one of `incoming` bytes.
	#[must_use]
	pub fn allows_upload(&self, stored_bytes: i64, incoming: usize) -> bool {
		let incoming = i64::try_from(incoming).unwrap_or(i64::MAX);
		self.quota_mb.is_none_or(|mb| stored_bytes.saturating_add(incoming) <= i64::from(mb) * 1024 * 1024)
	}
}

/// Admin setting for chat attachments; the admin PATCH takes the same shape. Off by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AttachmentSettings {
	/// Keep uploaded PDFs and send the original file to models that read PDFs, instead of only its extracted text.
	pub native_pdf: bool,
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
	/// Image ID of the instance icon, served without a session at `/api/v1/files/{id}`; `None` means the default logo.
	pub icon_id: Option<Uuid>,
}
