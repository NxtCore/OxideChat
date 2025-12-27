//! Base endpoint types.
//!
//! Types for the base application data endpoint.

use crate::config::OAuthProvider;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

// ============================================================================
// Response Types
// ============================================================================

/// Response containing base application data.
#[derive(Debug, Serialize)]
pub struct BaseResponse {
	pub i18n: Arc<Value>,
	pub needs_setup: bool,
	pub oauth_providers: Vec<OAuthProvider>,
}
