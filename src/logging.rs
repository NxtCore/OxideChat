//! Audit logging module for OxideChat.
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
	Role,
	Translation,
	Config,
	RateLimit,
}

impl EntityType {
	/// Get the entity type as a static string.
	#[must_use]
	pub const fn as_str(&self) -> &'static str {
		match self {
			Self::User => "user",
			Self::Session => "session",
			Self::Role => "role",
			Self::Translation => "translation",
			Self::Config => "config",
			Self::RateLimit => "rate_limit",
		}
	}

	/// Convert from string.
	#[must_use]
	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"user" => Some(Self::User),
			"session" => Some(Self::Session),
			"role" => Some(Self::Role),
			"translation" => Some(Self::Translation),
			"config" => Some(Self::Config),
			"rate_limit" => Some(Self::RateLimit),
			_ => None,
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
	SessionCreated,
	SessionExpired,
	SessionInvalidated,

	// User Management
	RoleAssigned,
	RoleRevoked,
	UserUpdated,
	UserDeleted,

	// Admin Actions
	AdminSetup,
	ConfigUpdated,
	TranslationCreated,
	TranslationUpdated,
	TranslationDeleted,

	// Security
	PasswordChanged,
	RateLimited,

	// OAuth
	OAuthLoginSuccess,
	OAuthLoginFailed,
	OAuthAccountCreated,
	OAuthIdentityLinked,
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
			Self::SessionCreated => "session_created",
			Self::SessionExpired => "session_expired",
			Self::SessionInvalidated => "session_invalidated",
			Self::RoleAssigned => "role_assigned",
			Self::RoleRevoked => "role_revoked",
			Self::UserUpdated => "user_updated",
			Self::UserDeleted => "user_deleted",
			Self::AdminSetup => "admin_setup",
			Self::ConfigUpdated => "config_updated",
			Self::TranslationCreated => "translation_created",
			Self::TranslationUpdated => "translation_updated",
			Self::TranslationDeleted => "translation_deleted",
			Self::PasswordChanged => "password_changed",
			Self::RateLimited => "rate_limited",
			Self::OAuthLoginSuccess => "oauth_login_success",
			Self::OAuthLoginFailed => "oauth_login_failed",
			Self::OAuthAccountCreated => "oauth_account_created",
			Self::OAuthIdentityLinked => "oauth_identity_linked",
		}
	}

	/// Convert from string.
	#[must_use]
	pub fn from_str(s: &str) -> Option<Self> {
		match s {
			"user_registered" => Some(Self::UserRegistered),
			"user_login" => Some(Self::UserLogin),
			"user_login_failed" => Some(Self::UserLoginFailed),
			"user_logout" => Some(Self::UserLogout),
			"session_created" => Some(Self::SessionCreated),
			"session_expired" => Some(Self::SessionExpired),
			"session_invalidated" => Some(Self::SessionInvalidated),
			"role_assigned" => Some(Self::RoleAssigned),
			"role_revoked" => Some(Self::RoleRevoked),
			"user_updated" => Some(Self::UserUpdated),
			"user_deleted" => Some(Self::UserDeleted),
			"admin_setup" => Some(Self::AdminSetup),
			"config_updated" => Some(Self::ConfigUpdated),
			"translation_created" => Some(Self::TranslationCreated),
			"translation_updated" => Some(Self::TranslationUpdated),
			"translation_deleted" => Some(Self::TranslationDeleted),
			"password_changed" => Some(Self::PasswordChanged),
			"rate_limited" => Some(Self::RateLimited),
			"oauth_login_success" => Some(Self::OAuthLoginSuccess),
			"oauth_login_failed" => Some(Self::OAuthLoginFailed),
			"oauth_account_created" => Some(Self::OAuthAccountCreated),
			"oauth_identity_linked" => Some(Self::OAuthIdentityLinked),
			_ => None,
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

	/// Log an event with actor, target, and resource entities.
	///
	/// Use this for complex operations like "User A assigns Role C to User B".
	pub fn log_full(
		pool: &PgPool,
		event: LogEvent,
		actor_id: Option<Uuid>,
		target_type: Option<EntityType>,
		target_id: Option<Uuid>,
		resource_type: Option<EntityType>,
		resource_id: Option<Uuid>,
	) {
		Self::log_internal(pool.clone(), event, actor_id, target_type, target_id, resource_type, resource_id, None);
	}

	/// Log an event with all fields including metadata.
	///
	/// Use this when you need to attach additional context as JSON.
	pub fn log_with_metadata(
		pool: &PgPool,
		event: LogEvent,
		actor_id: Option<Uuid>,
		target_type: Option<EntityType>,
		target_id: Option<Uuid>,
		resource_type: Option<EntityType>,
		resource_id: Option<Uuid>,
		metadata: Value,
	) {
		Self::log_internal(pool.clone(), event, actor_id, target_type, target_id, resource_type, resource_id, Some(metadata));
	}

	/// Internal logging implementation that spawns a fire-and-forget task.
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
