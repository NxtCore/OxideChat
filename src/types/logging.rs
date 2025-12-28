//! Logging-related types.
//!
//! Database row structs and response types for audit log queries.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

// ============================================================================
// Internal Types (DB rows)
// ============================================================================

/// Database row for audit logs.
#[derive(Debug, FromRow)]
pub struct AuditLogRow {
	pub id: Uuid,
	pub event: String,
	pub actor_id: Option<Uuid>,
	pub target_type: Option<String>,
	pub target_id: Option<Uuid>,
	pub resource_type: Option<String>,
	pub resource_id: Option<Uuid>,
	pub metadata: Option<Value>,
	pub created_at: DateTime<Utc>,
}

// ============================================================================
// Response Types
// ============================================================================

/// Response for admin audit log viewing.
#[derive(Debug, Serialize)]
pub struct AuditLogResponse {
	pub id: Uuid,
	pub event: String,
	pub actor_id: Option<Uuid>,
	pub target_type: Option<String>,
	pub target_id: Option<Uuid>,
	pub resource_type: Option<String>,
	pub resource_id: Option<Uuid>,
	pub metadata: Option<Value>,
	pub created_at: DateTime<Utc>,
}

impl From<AuditLogRow> for AuditLogResponse {
	fn from(row: AuditLogRow) -> Self {
		Self {
			id: row.id,
			event: row.event,
			actor_id: row.actor_id,
			target_type: row.target_type,
			target_id: row.target_id,
			resource_type: row.resource_type,
			resource_id: row.resource_id,
			metadata: row.metadata,
			created_at: row.created_at,
		}
	}
}
