use crate::routes::public::auth::get_current_user;
use crate::types::{JobState, RequestContext};
use crate::utils::rate_limit;
use crate::utils::response::{ErrorBuilder, ErrorCode};
use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use std::net::SocketAddr;
use std::sync::{Arc, LazyLock};
use tower_cookies::Cookies;

static TRUST_PROXY: LazyLock<bool> = LazyLock::new(|| std::env::var("TRUST_PROXY").is_ok_and(|value| value == "true"));

/// Loads the session user once and makes it available to downstream handlers.
pub async fn load_current_user(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	mut request: Request,
	next: Next,
) -> Response {
	let user = get_current_user(&state.db, &cookies).await;
	request.extensions_mut().insert(RequestContext { user });
	next.run(request).await
}

/// Returns the client IP, honoring `X-Forwarded-For` only when `TRUST_PROXY=true`.
#[must_use]
pub fn client_ip(request: &Request) -> String {
	if *TRUST_PROXY
		&& let Some(forwarded) = request.headers().get("x-forwarded-for").and_then(|value| value.to_str().ok())
		&& let Some(first) = forwarded.split(',').next().map(str::trim)
		&& !first.is_empty()
	{
		return first.to_string();
	}
	request
		.extensions()
		.get::<ConnectInfo<SocketAddr>>()
		.map(|connect_info| connect_info.0.to_string())
		.unwrap_or_else(|| "unknown".to_string())
}

/// Rate limits pre-authentication routes (login, register, setup, OAuth) per client IP.
pub async fn rate_limit(request: Request, next: Next) -> Response {
	let key = format!("auth:ip:{}", client_ip(&request));
	if !rate_limit::global().allow(&key, rate_limit::auth_ip_limit(), rate_limit::RATE_LIMIT_WINDOW).await {
		return ErrorBuilder::new(ErrorCode::TooManyRequests).build();
	}
	next.run(request).await
}
