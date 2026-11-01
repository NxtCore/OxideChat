//! Audit logging module for `OxideChat`.
//!
//! Provides async fire-and-forget logging for crucial application events.
//! Uses enums for type-safe event and entity type handling.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

// ============================================================================
// Entity Types
// ============================================================================

/// Entity types that can be referenced in audit logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
	User,
	Session,
	Translation,
}

impl EntityType {
	/// Get the entity type as a static string.
	#[must_use]
	pub const fn as_str(&self) -> &'static str {
		match self {
			Self::User => "user",
			Self::Session => "session",
			Self::Translation => "translation",
		}
	}
}

// ============================================================================
// Log Events
// ============================================================================

/// Event types that can be logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogEvent {
	// Authentication
	UserRegistered,
	UserLogin,
	UserLoginFailed,
	UserLogout,

	// User Management
	RoleAssigned,
	UserUpdated,
	UserDeleted,

	// Admin Actions
	AdminSetup,
	TranslationUpdated,
	TranslationDeleted,

	// Security
	PasswordChanged,

	// OAuth
	OAuthLoginSuccess,
	OAuthLoginFailed,
	OAuthAccountCreated,
}

impl LogEvent {
	/// Get the event code as a static string.
	#[must_use]
	pub const fn as_str(&self) -> &'static str {
		match self {
			Self::UserRegistered => "user_registered",
			Self::UserLogin => "user_login",
			Self::UserLoginFailed => "user_login_failed",
			Self::UserLogout => "user_logout",
			Self::RoleAssigned => "role_assigned",
			Self::UserUpdated => "user_updated",
			Self::UserDeleted => "user_deleted",
			Self::AdminSetup => "admin_setup",
			Self::TranslationUpdated => "translation_updated",
			Self::TranslationDeleted => "translation_deleted",
			Self::PasswordChanged => "password_changed",
			Self::OAuthLoginSuccess => "oauth_login_success",
			Self::OAuthLoginFailed => "oauth_login_failed",
			Self::OAuthAccountCreated => "oauth_account_created",
		}
	}
}

// ============================================================================
// Audit Logger
// ============================================================================

/// Audit logger for tracking application events.
///
/// All logging methods are fire-and-forget - they spawn a tokio task
/// and return immediately to avoid blocking request handlers.
pub struct AuditLog;

impl AuditLog {
	/// Log an event with actor and optional target entity.
	///
	/// This is the most common logging pattern for simple events like login/logout.
	pub fn log(pool: &PgPool, event: LogEvent, actor_id: Option<Uuid>, target_type: Option<EntityType>, target_id: Option<Uuid>) {
		Self::log_internal(pool.clone(), event, actor_id, target_type, target_id, None, None, None);
	}

	/// Internal logging implementation that spawns a fire-and-forget task.
	#[allow(clippy::too_many_arguments)]
	fn log_internal(
		pool: PgPool,
		event: LogEvent,
		actor_id: Option<Uuid>,
		target_type: Option<EntityType>,
		target_id: Option<Uuid>,
		resource_type: Option<EntityType>,
		resource_id: Option<Uuid>,
		metadata: Option<Value>,
	) {
		let event_str = event.as_str();
		let target_type_str = target_type.map(|t| t.as_str().to_string());
		let resource_type_str = resource_type.map(|t| t.as_str().to_string());

		tokio::spawn(async move {
			let result = sqlx::query(
				r#"
				INSERT INTO audit_logs (event, actor_id, target_type, target_id, resource_type, resource_id, metadata)
				VALUES ($1, $2, $3, $4, $5, $6, $7)
				"#,
			)
			.bind(event_str)
			.bind(actor_id)
			.bind(target_type_str)
			.bind(target_id)
			.bind(resource_type_str)
			.bind(resource_id)
			.bind(metadata)
			.execute(&pool)
			.await;

			if let Err(e) = result {
				eprintln!("[AUDIT] Failed to log event '{}': {}", event_str, e);
			}
		});
	}
}
