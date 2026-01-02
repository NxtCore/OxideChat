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
		// Admin i18n
		.route("/api/v1/admin/i18n", get(admin::i18n::list_translations))
		.route("/api/v1/admin/i18n/translations", put(admin::i18n::upsert_translation))
		.route("/api/v1/admin/i18n/translations/{id}", delete(admin::i18n::delete_translation))
		// Admin AI Providers
		.route("/api/v1/admin/providers", get(admin::providers::list_providers))
		.route("/api/v1/admin/providers", post(admin::providers::create_provider))
		.route("/api/v1/admin/providers/test", post(admin::providers::test_provider_inline))
		.route("/api/v1/admin/providers/{id}", get(admin::providers::get_provider))
		.route("/api/v1/admin/providers/{id}", put(admin::providers::update_provider))
		.route("/api/v1/admin/providers/{id}", delete(admin::providers::delete_provider))
		.route("/api/v1/admin/providers/{id}/test", post(admin::providers::test_provider))
		.route("/api/v1/admin/providers/{id}/sync", post(admin::providers::sync_provider))
		.route("/api/v1/admin/providers/{id}/models", get(admin::providers::list_models))
		// Auth
		.route("/api/v1/auth/setup", post(public::auth::setup))
		.route("/api/v1/auth/register", post(public::auth::register))
		.route("/api/v1/auth/login", post(public::auth::login))
		.route("/api/v1/auth/logout", post(public::auth::logout))
		.route("/api/v1/auth/oauth/{provider}", get(public::oauth::oauth_init))
		.route("/api/v1/auth/oauth/{provider}/callback", get(public::oauth::oauth_callback))
		// Users
		.route("/api/v1/users/@me", get(public::users::get_me))
		.layer(CookieManagerLayer::new())
}
