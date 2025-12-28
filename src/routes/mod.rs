//! Route handlers for OxideChat API.

pub mod admin;
pub mod public;

use crate::AppState;
use axum::{
	Router,
	routing::{delete, get, post, put},
};
use std::sync::Arc;
use tower_cookies::CookieManagerLayer;

pub fn build_router() -> Router<Arc<AppState>> {
	Router::new()
		.route("/api/v1/base", get(public::base::get_base))
		.route("/api/v1/admin/i18n", get(admin::i18n::list_translations))
		.route("/api/v1/admin/i18n/translations", put(admin::i18n::upsert_translation))
		.route("/api/v1/admin/i18n/translations/{id}", delete(admin::i18n::delete_translation))
		.route("/api/v1/auth/setup", post(public::auth::setup))
		.route("/api/v1/auth/register", post(public::auth::register))
		.route("/api/v1/auth/login", post(public::auth::login))
		.route("/api/v1/auth/logout", post(public::auth::logout))
		.route("/api/v1/auth/oauth/{provider}", get(public::oauth::oauth_init))
		.route("/api/v1/auth/oauth/{provider}/callback", get(public::oauth::oauth_callback))
		.route("/api/v1/users/@me", get(public::users::get_me))
		.layer(CookieManagerLayer::new())
}
