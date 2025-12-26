//! I18n-related types.
//!
//! Request and response types for translation management.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ============================================================================
// Request Types
// ============================================================================

/// Request body for creating or updating a translation.
#[derive(Debug, Deserialize)]
pub struct UpsertTranslationRequest {
	pub language: String,
	pub key_path: String,
	pub value: String,
	#[serde(default)]
	pub is_override: bool,
}

// ============================================================================
// Response Types
// ============================================================================

/// A single translation entry.
#[derive(Debug, Serialize, FromRow)]
pub struct Translation {
	pub id: sqlx::types::Uuid,
	pub language: String,
	pub key_path: String,
	pub value: String,
	pub is_override: bool,
}

/// Response containing a list of translations.
#[derive(Debug, Serialize)]
pub struct TranslationsResponse {
	pub translations: Vec<Translation>,
}

// ============================================================================
// Internal Types
// ============================================================================

/// Helper struct for extracting just the ID from a query result.
#[derive(FromRow)]
pub struct IdRow {
	pub id: sqlx::types::Uuid,
}
