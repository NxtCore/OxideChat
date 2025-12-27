//! Route handlers for OxideChat API.

pub mod admin;
pub mod auth;
pub mod base;
pub mod users;

use crate::AppState;
use axum::{
	Router,
	routing::{delete, get, post, put},
};
use std::sync::Arc;
use tower_cookies::CookieManagerLayer;

/// Build the API router with all routes.
pub fn build_router() -> Router<Arc<AppState>> {
	let admin_i18n_routes = Router::new()
		.route("/", get(admin::i18n::list_translations))
		.route("/translations", put(admin::i18n::upsert_translation))
		.route("/translations/{id}", delete(admin::i18n::delete_translation));

	let admin_routes = Router::new().nest("/i18n", admin_i18n_routes);

	let auth_routes = Router::new()
		.route("/setup", post(auth::setup))
		.route("/register", post(auth::register))
		.route("/login", post(auth::login))
		.route("/logout", post(auth::logout));

	let users_routes = Router::new().route("/@me", get(users::get_me));

	let v1_routes = Router::new()
		.route("/base", get(base::get_base))
		.nest("/admin", admin_routes)
		.nest("/auth", auth_routes)
		.nest("/users", users_routes);

	Router::new().nest("/api/v1", v1_routes).layer(CookieManagerLayer::new())
}
