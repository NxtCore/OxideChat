//! Base endpoint types.
//!
//! Types for the base application data endpoint.

use serde::Serialize;
use serde_json::Value;

// ============================================================================
// Response Types
// ============================================================================

/// Response containing base application data.
#[derive(Debug, Serialize)]
pub struct BaseResponse {
	pub i18n: Value,
}
