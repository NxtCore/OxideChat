use crate::logging::{AuditLog, EntityType, LogEvent};
use crate::types::{AppError, AuthResponse, JobState, LoginRequest, RegisterRequest, SetupRequest, User, is_unique_violation};
use crate::utils::auth::{SESSION_COOKIE_NAME, create_session, hash_password, validate_email, validate_password, validate_username, verify_password};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::{
	Json,
	extract::{ConnectInfo, State},
	http::StatusCode,
	response::Response,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_cookies::{Cookie, Cookies};
use uuid::Uuid;

fn registration_error(error: sqlx::Error) -> AppError {
	if !is_unique_violation(&error) {
		return AppError::database(error);
	}
	match error.as_database_error().and_then(|database_error| database_error.constraint()) {
		Some("users_email_key") => ErrorCode::EmailTaken.into(),
		Some("users_username_key") => ErrorCode::UsernameTaken.into(),
		_ => ErrorCode::EmailOrUsernameTaken.into(),
	}
}

fn validate_credentials(email: &str, username: &str, password: &str) -> Result<(), ErrorCode> {
	if !validate_email(email) {
		return Err(ErrorCode::InvalidEmail);
	}
	validate_username(username)?;
	validate_password(password)
}

fn hash_new_password(password: &str) -> Result<String, ErrorCode> {
	hash_password(password).map_err(|_| ErrorCode::InternalError)
}

async fn finish_signup(state: &JobState, cookies: &Cookies, user: &User) -> Result<(), AppError> {
	user.initialize_defaults(&state.db).await.map_err(AppError::database)?;
	create_session(&state.db, cookies, &user.id).await.map_err(AppError::database)
}

fn created(user_response: crate::types::UserResponse) -> Response {
	ResponseBuilder::new(ResponseBody::Json(AuthResponse { user: user_response })).status(StatusCode::CREATED).build()
}

/// POST /api/v1/auth/setup
pub async fn setup(State(state): State<Arc<JobState>>, cookies: Cookies, Json(payload): Json<SetupRequest>) -> Result<Response, AppError> {
	if User::any_exist(&state.db).await.map_err(AppError::database)? {
		return Err(ErrorCode::SetupCompleted.into());
	}

	validate_credentials(&payload.email, &payload.username, &payload.password)?;
	let password_hash = hash_new_password(&payload.password)?;

	let user = User::create_first_admin(&state.db, &payload.email, &payload.username, &password_hash)
		.await
		.map_err(AppError::database)?
		.ok_or(ErrorCode::SetupCompleted)?;

	finish_signup(&state, &cookies, &user).await?;

	AuditLog::log(&state.db, LogEvent::AdminSetup, Some(user.id), Some(EntityType::User), Some(user.id));

	Ok(created(user.to_response(&state.db).await?))
}

/// POST /api/v1/auth/register
pub async fn register(State(state): State<Arc<JobState>>, cookies: Cookies, Json(payload): Json<RegisterRequest>) -> Result<Response, AppError> {
	if !User::any_exist(&state.db).await.map_err(AppError::database)? {
		return Err(ErrorCode::SetupRequired.into());
	}

	validate_credentials(&payload.email, &payload.username, &payload.password)?;
	let password_hash = hash_new_password(&payload.password)?;

	let user = User::create(&state.db, &payload.email, &payload.username, &password_hash)
		.await
		.map_err(registration_error)?;

	user.assign_role(&state.db, "user").await.map_err(AppError::database)?;
	finish_signup(&state, &cookies, &user).await?;

	AuditLog::log(&state.db, LogEvent::UserRegistered, Some(user.id), Some(EntityType::User), Some(user.id));

	Ok(created(user.to_response(&state.db).await?))
}

/// POST /api/v1/auth/login
pub async fn login(
	State(state): State<Arc<JobState>>,
	ConnectInfo(addr): ConnectInfo<SocketAddr>,
	cookies: Cookies,
	Json(payload): Json<LoginRequest>,
) -> Result<Response, AppError> {
	let ip = addr.ip();
	if !state.login_limiter.allow(ip, &payload.email).await {
		AuditLog::log(&state.db, LogEvent::UserLoginFailed, None, None, None);
		return Err(ErrorCode::RateLimited.into());
	}

	let Some(user) = User::find_by_email(&state.db, &payload.email).await.map_err(AppError::database)? else {
		state.login_limiter.record_failure(ip, &payload.email);
		AuditLog::log(&state.db, LogEvent::UserLoginFailed, None, None, None);
		return Err(ErrorCode::InvalidCredentials.into());
	};

	let Some(password_hash) = &user.password_hash else {
		state.login_limiter.record_failure(ip, &payload.email);
		return Err(ErrorCode::ExternalAuthRequired.into());
	};

	if !verify_password(&payload.password, password_hash).map_err(|_| ErrorCode::InternalError)? {
		state.login_limiter.record_failure(ip, &payload.email);
		AuditLog::log(&state.db, LogEvent::UserLoginFailed, Some(user.id), Some(EntityType::User), Some(user.id));
		return Err(ErrorCode::InvalidCredentials.into());
	}

	create_session(&state.db, &cookies, &user.id).await.map_err(AppError::database)?;

	AuditLog::log(&state.db, LogEvent::UserLogin, Some(user.id), Some(EntityType::Session), None);

	Ok(ResponseBuilder::new(ResponseBody::Json(AuthResponse { user: user.to_response(&state.db).await? })).build())
}

/// POST /api/v1/auth/logout
pub async fn logout(State(state): State<Arc<JobState>>, cookies: Cookies) -> Result<Response, AppError> {
	let mut user_id: Option<Uuid> = None;
	let mut session_id_to_log: Option<Uuid> = None;

	if let Some(session_cookie) = cookies.get(SESSION_COOKIE_NAME)
		&& let Ok(session_id) = Uuid::parse_str(session_cookie.value())
	{
		session_id_to_log = Some(session_id);
		user_id = User::session_owner(&state.db, &session_id).await.map_err(AppError::database)?;
		User::delete_session(&state.db, &session_id).await.map_err(AppError::database)?;
	}

	cookies.remove(Cookie::from(SESSION_COOKIE_NAME));

	AuditLog::log(&state.db, LogEvent::UserLogout, user_id, Some(EntityType::Session), session_id_to_log);

	Ok(ResponseBuilder::new(ResponseBody::Json(serde_json::json!({
		"message": crate::i18n::I18n::get().translate("auth.messages.logout_success", &None)
	})))
	.build())
}
