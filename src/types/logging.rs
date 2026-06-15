use crate::types::BaseType;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct AuditLog {
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

impl From<AuditLog> for AuditLogResponse {
	fn from(row: AuditLog) -> Self {
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

impl BaseType for AuditLog {}

impl AuditLog {
	pub async fn create(
		pool: &sqlx::PgPool,
		event: &str,
		actor_id: Option<&Uuid>,
		target_type: Option<&str>,
		target_id: Option<&Uuid>,
		resource_type: Option<&str>,
		resource_id: Option<&Uuid>,
		metadata: Option<&Value>,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, AuditLog>(
			r#"
			INSERT INTO audit_logs (event, actor_id, target_type, target_id, resource_type, resource_id, metadata)
			VALUES ($1, $2, $3, $4, $5, $6, $7)
			RETURNING *
			"#,
		)
		.bind(event)
		.bind(actor_id)
		.bind(target_type)
		.bind(target_id)
		.bind(resource_type)
		.bind(resource_id)
		.bind(metadata)
		.fetch_one(pool)
		.await
	}

	pub async fn list_paginated(
		pool: &sqlx::PgPool,
		limit: i64,
		offset: i64,
		event_filter: Option<&str>,
		actor_filter: Option<&Uuid>,
	) -> Result<(Vec<AuditLog>, i64), sqlx::Error> {
		let count: (i64,) = sqlx::query_as(
			"SELECT COUNT(*) FROM audit_logs WHERE ($1::text IS NULL OR event = $1) AND ($2::uuid IS NULL OR actor_id = $2)",
		)
		.bind(event_filter)
		.bind(actor_filter)
		.fetch_one(pool)
		.await?;

		let rows = sqlx::query_as::<_, AuditLog>(
			r#"
			SELECT * FROM audit_logs
			WHERE ($1::text IS NULL OR event = $1) AND ($2::uuid IS NULL OR actor_id = $2)
			ORDER BY created_at DESC
			LIMIT $3 OFFSET $4
			"#,
		)
		.bind(event_filter)
		.bind(actor_filter)
		.bind(limit)
		.bind(offset)
		.fetch_all(pool)
		.await?;

		Ok((rows, count.0))
	}
}
