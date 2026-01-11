//! Route handlers for OxideChat API.

pub mod admin;
pub mod public;

use crate::AppState;
use axum::{
	Router,
	routing::{delete, get, patch, post, put},
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
		// Admin Tools
		.route("/api/v1/admin/tools", get(admin::tools::list_tools))
		.route("/api/v1/admin/tools", post(admin::tools::create_tool))
		.route("/api/v1/admin/tools/wasm/upload", post(admin::tools::upload_wasm))
		.route("/api/v1/admin/tools/{id}", get(admin::tools::get_tool))
		.route("/api/v1/admin/tools/{id}", put(admin::tools::update_tool))
		.route("/api/v1/admin/tools/{id}", delete(admin::tools::delete_tool))
		.route("/api/v1/admin/tools/{id}/settings", get(admin::tools::get_tool_settings))
		.route("/api/v1/admin/tools/{id}/settings", put(admin::tools::set_tool_settings))
		.route("/api/v1/admin/tools/{id}/test", post(admin::tools::test_tool))
		// Admin Config
		.route("/api/v1/admin/config", patch(admin::config::update_global_config))
		// Public Config
		.route("/api/v1/config", get(admin::config::get_global_config))
		// Auth
		.route("/api/v1/auth/setup", post(public::auth::setup))
		.route("/api/v1/auth/register", post(public::auth::register))
		.route("/api/v1/auth/login", post(public::auth::login))
		.route("/api/v1/auth/logout", post(public::auth::logout))
		.route("/api/v1/auth/oauth/{provider}", get(public::oauth::oauth_init))
		.route("/api/v1/auth/oauth/{provider}/callback", get(public::oauth::oauth_callback))
		// Users
		.route("/api/v1/users/@me", get(public::users::get_me))
		.route("/api/v1/users/@me/preferences", get(public::preferences::get_preferences))
		.route("/api/v1/users/@me/preferences", patch(public::preferences::update_preferences))
		// Workspaces
		.route("/api/v1/workspaces", get(public::workspaces::list_workspaces))
		.route("/api/v1/workspaces", post(public::workspaces::create_workspace))
		.route("/api/v1/workspaces/{id}", get(public::workspaces::get_workspace))
		.route("/api/v1/workspaces/{id}", patch(public::workspaces::update_workspace))
		.route("/api/v1/workspaces/{id}", delete(public::workspaces::delete_workspace))
		// Chats
		.route("/api/v1/chats", get(public::chats::list_chats))
		.route("/api/v1/chats", post(public::chats::create_chat))
		.route("/api/v1/chats/{id}", get(public::chats::get_chat))
		.route("/api/v1/chats/{id}", patch(public::chats::update_chat))
		.route("/api/v1/chats/{id}", delete(public::chats::delete_chat))
		// Messages
		.route("/api/v1/chats/{chat_id}/messages", get(public::messages::list_messages))
		.route("/api/v1/chats/{chat_id}/messages", post(public::messages::send_message))
		.route("/api/v1/chats/{chat_id}/stream", post(public::streaming::stream_completion))
		// Models
		.route("/api/v1/models", get(public::models::list_models))
		// Tools
		.route("/api/v1/tools", get(public::tools::list_tools))
		.layer(CookieManagerLayer::new())
}
