//! Route handlers for `OxideChat` API.

pub mod admin;
pub mod gateway;
pub mod public;

use crate::types::JobState;
use axum::{
	Router,
	extract::DefaultBodyLimit,
	middleware::{from_fn, from_fn_with_state},
	routing::{get, post},
};
use std::sync::Arc;
use tower_cookies::CookieManagerLayer;

/// Builds the complete API router backed by shared [`Arc<JobState>`] state.
///
/// The returned router includes public, administrative, and OpenAI-compatible
/// gateway routes and owns a shared reference to the supplied application state.
pub fn build_router(state: Arc<JobState>) -> Router {
	let openai_read_router = Router::new()
		.route("/models", get(public::openai::list_models))
		.route_layer(from_fn_with_state(state.clone(), crate::routes::gateway::authenticate_read));
	let openai_write_router = Router::new()
		.route("/chat/completions", post(public::openai::chat_completions))
		.route("/responses", post(public::openai::responses))
		.route_layer(from_fn_with_state(state.clone(), crate::routes::gateway::authenticate_write));
	let openai_router = openai_read_router
		.merge(openai_write_router)
		.method_not_allowed_fallback(public::openai::method_not_allowed)
		.fallback(public::openai::not_found)
		.layer(from_fn(crate::routes::gateway::add_request_id));
	let auth_router = Router::new()
		.route("/setup", post(public::auth::setup))
		.route("/register", post(public::auth::register))
		.route("/login", post(public::auth::login))
		.route("/oauth/{provider}", get(public::oauth::oauth_init))
		.route("/oauth/{provider}/callback", get(public::oauth::oauth_callback))
		.route_layer(from_fn(crate::middleware::rate_limit))
		.route("/logout", post(public::auth::logout))
		.layer(DefaultBodyLimit::max(64 * 1024));
	Router::new()
		.nest("/openai/v1", openai_router)
		.nest("/api/v1/auth", auth_router)
		.merge(admin::router())
		.merge(public::router())
		.layer(from_fn_with_state(state.clone(), crate::middleware::load_current_user))
		.layer(CookieManagerLayer::new())
		.layer(crate::middleware::cors_layer())
		.layer(from_fn(crate::middleware::security_headers))
		.with_state(state)
}
