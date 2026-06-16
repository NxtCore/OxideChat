use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub(super) struct WorkspaceWithCount {
	pub id: Uuid,
	pub user_id: Uuid,
	pub name: String,
	pub icon: Option<String>,
	pub color: Option<String>,
	pub sort_order: i32,
	pub is_default: bool,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub chat_count: i64,
}

#[derive(Debug, FromRow)]
pub(super) struct SiblingCountRow {
	pub parent_id: Option<Uuid>,
	pub role: String,
	pub count: i64,
}

#[derive(Debug, FromRow)]
pub(super) struct ToolExecutionRow {
	pub id: Uuid,
	pub message_id: Option<Uuid>,
	pub tool_call_id: String,
	pub input_args: Value,
	pub output: Option<Value>,
	pub error: Option<String>,
	pub execution_ms: Option<i32>,
	pub tool_id: Option<Uuid>,
	pub tool_function: Option<Uuid>,
	pub tool_name: Option<String>,
}
