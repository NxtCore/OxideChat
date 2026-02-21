//! Base endpoint types.
//!
//! Types for the base application data endpoint.

use crate::config::OAuthProvider;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use crate::types::Role;
// ============================================================================
// Response Types
// ============================================================================

/// Response containing base application data.
#[derive(Debug, Serialize)]
pub struct BaseResponse {
	pub i18n: Arc<Value>,
	pub language: String,
	pub needs_setup: bool,
	pub oauth_providers: Vec<OAuthProvider>,
	pub roles: Vec<Role>,
}
