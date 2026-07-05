use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::types::JobState;
use crate::types::{AuthResponse, LoginRequest, RegisterRequest, SetupRequest, User};
use crate::utils::auth::{create_session, hash_password, validate_email, validate_password, validate_username, verify_password};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::{Json, extract::State, response::IntoResponse};
use sqlx::PgPool;
use std::sync::Arc;
use tower_cookies::{Cookie, Cookies};
use uuid::Uuid;

pub const SESSION_COOKIE_NAME: &str = "oxidechat_session";
pub const SESSION_DURATION_DAYS: i64 = 7;
pub const MIN_PASSWORD_LENGTH: usize = 8;
pub const MAX_PASSWORD_LENGTH: usize = 128;
pub const MAX_USERNAME_LENGTH: usize = 32;
pub const MIN_USERNAME_LENGTH: usize = 3;

/// POST /api/v1/auth/setup
pub async fn setup(State(state): State<Arc<JobState>>, cookies: Cookies, Json(payload): Json<SetupRequest>) -> impl IntoResponse {
	match User::any_exist(&state.db).await {
		Ok(true) => return ErrorBuilder::new(ErrorCode::SetupCompleted).build(),
		Err(e) => {
			eprintln!("[AUTH] Database error checking users: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
		_ => {}
	}

	if !validate_email(&payload.email) {
		return ErrorBuilder::new(ErrorCode::InvalidEmail).build();
	}
	if let Err(code) = validate_username(&payload.username) {
		return ErrorBuilder::new(code).build();
	}
	if let Err(code) = validate_password(&payload.password) {
		return ErrorBuilder::new(code).build();
	}

	let password_hash = match hash_password(&payload.password) {
		Ok(hash) => hash,
		Err(e) => {
			eprintln!("[AUTH] Password hashing error: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let user = match User::create(&state.db, &payload.email, &payload.username, &password_hash).await {
		Ok(u) => u,
		Err(e) => {
			eprintln!("[AUTH] Failed to create user: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	if let Err(e) = user.assign_role(&state.db, "admin").await {
		eprintln!("[AUTH] Failed to assign admin role: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	if let Err(e) = user.initialize_defaults(&state.db).await {
		eprintln!("[AUTH] Failed to initialize user defaults: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
		eprintln!("[AUTH] Failed to create session: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	AuditLog::log(&state.db, LogEvent::AdminSetup, Some(user.id), Some(EntityType::User), Some(user.id));

	match user.to_response(&state.db).await {
		Ok(user_response) => ResponseBuilder::new(ResponseBody::Json(AuthResponse { user: user_response }))
			.status(axum::http::StatusCode::CREATED)
			.build(),
		Err(e) => {
			eprintln!("[AUTH] Failed to fetch user roles: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/auth/register
pub async fn register(State(state): State<Arc<JobState>>, cookies: Cookies, Json(payload): Json<RegisterRequest>) -> impl IntoResponse {
	match User::any_exist(&state.db).await {
		Ok(false) => return ErrorBuilder::new(ErrorCode::SetupRequired).build(),
		Err(e) => {
			eprintln!("[AUTH] Database error checking users: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
		_ => {}
	}

	if !validate_email(&payload.email) {
		return ErrorBuilder::new(ErrorCode::InvalidEmail).build();
	}
	if let Err(code) = validate_username(&payload.username) {
		return ErrorBuilder::new(code).build();
	}
	if let Err(code) = validate_password(&payload.password) {
		return ErrorBuilder::new(code).build();
	}

	let password_hash = match hash_password(&payload.password) {
		Ok(hash) => hash,
		Err(e) => {
			eprintln!("[AUTH] Password hashing error: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let user = match User::create(&state.db, &payload.email, &payload.username, &password_hash).await {
		Ok(u) => u,
		Err(e) => {
			let error_string = e.to_string();
			let code = if error_string.contains("duplicate key") {
				if error_string.contains("users_email_key") {
					ErrorCode::EmailTaken
				} else if error_string.contains("users_username_key") {
					ErrorCode::UsernameTaken
				} else {
					ErrorCode::EmailOrUsernameTaken
				}
			} else {
				eprintln!("[AUTH] Database error during registration: {e}");
				ErrorCode::DatabaseError
			};
			return ErrorBuilder::new(code).build();
		}
	};

	if let Err(e) = user.assign_role(&state.db, "user").await {
		eprintln!("[AUTH] Failed to assign user role: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	if let Err(e) = user.initialize_defaults(&state.db).await {
		eprintln!("[AUTH] Failed to initialize user defaults: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
		eprintln!("[AUTH] Failed to create session: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	AuditLog::log(&state.db, LogEvent::UserRegistered, Some(user.id), Some(EntityType::User), Some(user.id));

	match user.to_response(&state.db).await {
		Ok(user_response) => ResponseBuilder::new(ResponseBody::Json(AuthResponse { user: user_response }))
			.status(axum::http::StatusCode::CREATED)
			.build(),
		Err(e) => {
			eprintln!("[AUTH] Failed to fetch user roles: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/auth/login
pub async fn login(State(state): State<Arc<JobState>>, cookies: Cookies, Json(payload): Json<LoginRequest>) -> impl IntoResponse {
	let user = match User::find_by_email(&state.db, &payload.email).await {
		Ok(Some(user)) => user,
		Ok(None) => {
			AuditLog::log(&state.db, LogEvent::UserLoginFailed, None, None, None);
			return ErrorBuilder::new(ErrorCode::InvalidCredentials).build();
		}
		Err(e) => {
			eprintln!("[AUTH] Database error during login: {e}");
			return ErrorBuilder::new(ErrorCode::DatabaseError).build();
		}
	};

	let Some(password_hash) = &user.password_hash else {
		return ErrorBuilder::new(ErrorCode::ExternalAuthRequired).build();
	};

	match verify_password(&payload.password, password_hash) {
		Ok(true) => {}
		Ok(false) => {
			AuditLog::log(&state.db, LogEvent::UserLoginFailed, Some(user.id), Some(EntityType::User), Some(user.id));
			return ErrorBuilder::new(ErrorCode::InvalidCredentials).build();
		}
		Err(e) => {
			eprintln!("[AUTH] Password verification error: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	if let Err(e) = create_session(&state.db, &cookies, &user.id).await {
		eprintln!("[AUTH] Failed to create session: {e}");
		return ErrorBuilder::new(ErrorCode::DatabaseError).build();
	}

	AuditLog::log(&state.db, LogEvent::UserLogin, Some(user.id), Some(EntityType::Session), None);

	match user.to_response(&state.db).await {
		Ok(user_response) => ResponseBuilder::new(ResponseBody::Json(AuthResponse { user: user_response })).build(),
		Err(e) => {
			eprintln!("[AUTH] Failed to fetch user roles: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// POST /api/v1/auth/logout
pub async fn logout(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let mut user_id: Option<Uuid> = None;
	let mut session_id_to_log: Option<Uuid> = None;

	if let Some(session_cookie) = cookies.get(SESSION_COOKIE_NAME) {
		if let Ok(session_id) = Uuid::parse_str(session_cookie.value()) {
			session_id_to_log = Some(session_id);

			match sqlx::query_scalar::<_, Uuid>("SELECT user_id FROM sessions WHERE id = $1")
				.bind(session_id)
				.fetch_optional(&state.db)
				.await
			{
				Ok(Some(uid)) => user_id = Some(uid),
				Ok(None) => {}
				Err(e) => {
					eprintln!("[AUTH] Database error during logout session lookup: {e}");
					return ErrorBuilder::new(ErrorCode::DatabaseError).build();
				}
			}

			if let Err(e) = sqlx::query("DELETE FROM sessions WHERE id = $1").bind(session_id).execute(&state.db).await {
				eprintln!("[AUTH] Database error deleting session: {e}");
				return ErrorBuilder::new(ErrorCode::DatabaseError).build();
			}
		}
	}

	cookies.remove(Cookie::from(SESSION_COOKIE_NAME));

	AuditLog::log(&state.db, LogEvent::UserLogout, user_id, Some(EntityType::Session), session_id_to_log);

	ResponseBuilder::new(ResponseBody::Json(serde_json::json!({
		"message": crate::i18n::I18n::get().translate("auth.messages.logout_success", &None)
	})))
	.build()
}

/// Get the current user from the session cookie.
pub async fn get_current_user(pool: &PgPool, cookies: &Cookies) -> Option<User> {
	let session_cookie = cookies.get(SESSION_COOKIE_NAME)?;
	let session_id = Uuid::parse_str(session_cookie.value()).ok()?;
	User::find_by_session(pool, &session_id).await.ok()?
}
