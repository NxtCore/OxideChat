//! Route handlers for OxideChat API.

pub mod admin;
pub mod public;

use crate::types::JobState;
use axum::{
	Router,
	routing::{delete, get, patch, post, put},
};
use std::sync::Arc;
use tower_cookies::CookieManagerLayer;

pub fn build_router() -> Router<Arc<JobState>> {
	let openai_router = Router::new()
		.route("/models", get(public::openai::list_models))
		.route("/chat/completions", post(public::openai::chat_completions))
		.route("/responses", post(public::openai::responses))
		.method_not_allowed_fallback(public::openai::method_not_allowed)
		.fallback(public::openai::not_found);
	Router::new()
		.nest("/openai/v1", openai_router)
		.route("/api/v1/health", get(public::base::health))
		.route("/api/v1/base", get(public::base::get_base))
		// Admin i18n
		.route("/api/v1/admin/i18n", get(admin::i18n::list_translations))
		.route("/api/v1/admin/i18n/translations", put(admin::i18n::upsert_translation))
		.route("/api/v1/admin/i18n/translations/{id}", delete(admin::i18n::delete_translation))
		// Admin AI Providers
		.route("/api/v1/admin/providers", get(admin::providers::list_providers))
		.route("/api/v1/admin/providers", post(admin::providers::create_provider))
		.route("/api/v1/admin/providers/billing", get(admin::provider_billing::list_billing))
		.route(
			"/api/v1/admin/providers/{id}/billing",
			get(admin::provider_billing::get_billing)
				.put(admin::provider_billing::update_billing)
				.delete(admin::provider_billing::delete_billing),
		)
		.route("/api/v1/admin/providers/{id}/billing/refresh", post(admin::provider_billing::refresh_billing))
		.route("/api/v1/admin/providers/test", post(admin::providers::test_provider_inline))
		.route("/api/v1/admin/providers/{id}", get(admin::providers::get_provider))
		.route("/api/v1/admin/providers/{id}", put(admin::providers::update_provider))
		.route("/api/v1/admin/providers/{id}", delete(admin::providers::delete_provider))
		.route("/api/v1/admin/providers/{id}/test", post(admin::providers::test_provider))
		.route("/api/v1/admin/providers/{id}/sync", post(admin::providers::sync_provider))
		.route("/api/v1/admin/providers/{id}/models", get(admin::providers::list_models))
		.route("/api/v1/admin/providers/{id}/catalog", get(admin::catalog::list_catalog))
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
		// Admin MCP Servers
		.route("/api/v1/admin/mcp-servers", get(admin::mcp::list_servers))
		.route("/api/v1/admin/mcp-servers", post(admin::mcp::create_server))
		.route("/api/v1/admin/mcp-servers/{id}", get(admin::mcp::get_server))
		.route("/api/v1/admin/mcp-servers/{id}", put(admin::mcp::update_server))
		.route("/api/v1/admin/mcp-servers/{id}", delete(admin::mcp::delete_server))
		.route("/api/v1/admin/mcp-servers/{id}/discover", post(admin::mcp::discover_server))
		.route("/api/v1/admin/mcp-servers/{id}/health-check", post(admin::mcp::health_check))
		// Admin Teams
		.route("/api/v1/admin/teams", get(admin::teams::list_teams))
		.route("/api/v1/admin/teams", post(admin::teams::create_team))
		.route("/api/v1/admin/teams/{id}", get(admin::teams::get_team))
		.route("/api/v1/admin/teams/{id}", patch(admin::teams::update_team))
		.route("/api/v1/admin/teams/{id}", delete(admin::teams::delete_team))
		.route("/api/v1/admin/teams/{id}/members", put(admin::teams::set_team_members))
		.route("/api/v1/admin/teams/{id}/models", put(admin::teams::set_team_models))
		.route("/api/v1/admin/teams/{id}/budget", patch(admin::teams::update_team_budget))
		// Admin Budgets
		.route("/api/v1/admin/budgets", get(admin::budgets::list_budgets))
		.route("/api/v1/admin/budgets", post(admin::budgets::create_budget))
		.route("/api/v1/admin/budgets/overview/users", get(admin::budgets::user_overview))
		.route("/api/v1/admin/budgets/overview/teams", get(admin::budgets::team_overview))
		.route("/api/v1/admin/budgets/resets", get(admin::budgets::reset_history).post(admin::budgets::reset_budget))
		.route("/api/v1/admin/budgets/{id}", patch(admin::budgets::update_budget))
		.route("/api/v1/admin/budgets/{id}", delete(admin::budgets::delete_budget))
		.route(
			"/api/v1/admin/budgets/{id}/assignments",
			get(admin::budgets::get_budget_assignments).post(admin::budgets::assign_budget),
		)
		.route("/api/v1/admin/budgets/{id}/assignments/{assignment_id}", delete(admin::budgets::delete_assignment))
		.route("/api/v1/admin/analytics", get(admin::analytics::get_analytics))
		// Admin Users
		.route("/api/v1/admin/users", get(admin::users::list_users))
		.route("/api/v1/admin/users", post(admin::users::create_user))
		.route("/api/v1/admin/users/{id}", get(admin::users::get_user))
		.route("/api/v1/admin/users/{id}", put(admin::users::update_user))
		.route("/api/v1/admin/users/{id}", delete(admin::users::delete_user))
		.route("/api/v1/admin/users/{id}/roles", put(admin::users::set_user_roles))
		.route("/api/v1/admin/users/{id}/teams", put(admin::users::set_user_teams))
		.route("/api/v1/admin/users/{id}/password", put(admin::users::reset_password))
		// Admin Models
		.route("/api/v1/admin/models", get(admin::models::list_models))
		.route("/api/v1/admin/image-models", get(admin::models::list_image_models))
		.route("/api/v1/admin/image-model-providers", get(admin::models::list_image_model_providers))
		.route("/api/v1/admin/models/{id}", get(admin::models::get_model))
		.route("/api/v1/admin/models/{id}", patch(admin::models::patch_model))
		.route("/api/v1/admin/models/{id}/pricing", get(admin::models::get_model_pricing))
		.route("/api/v1/admin/models/{id}/pricing", put(admin::models::put_model_pricing))
		.route("/api/v1/admin/models/{id}/pricing", delete(admin::models::delete_model_pricing))
		.route("/api/v1/admin/models/{id}/provider-options", get(admin::catalog::list_provider_options))
		.route("/api/v1/admin/models/{id}/provider-options/refresh", post(admin::catalog::refresh_provider_options))
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
		.route("/api/v1/me/budget", get(public::users::get_my_budget))
		.route("/api/v1/me/analytics", get(public::users::get_my_analytics))
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
		.route("/api/v1/chats/{chat_id}/stream/tool-result", post(public::streaming::submit_client_tool_result))
		// Message Forks
		.route("/api/v1/chats/{chat_id}/messages/{message_id}/edit", post(public::messages::edit_message))
		.route("/api/v1/chats/{chat_id}/messages/{message_id}/switch-fork", post(public::messages::switch_fork))
		.route("/api/v1/chats/{chat_id}/messages/{message_id}/siblings", get(public::messages::get_siblings))
		.route("/api/v1/chats/{chat_id}/messages/{message_id}/fork", delete(public::messages::delete_fork))
		.route("/api/v1/chats/{chat_id}/messages/{message_id}/branch", post(public::messages::branch_from_message))
		// Models
		.route("/api/v1/models", get(public::models::list_models))
		.route("/api/v1/models/{id}/favorite", post(public::models::set_model_favorite))
		.route("/api/v1/models/{id}/provider-options", get(public::models::get_provider_options))
		// Providers
		.route("/api/v1/providers", get(public::providers::list_providers))
		// Tools
		.route("/api/v1/tools", get(public::tools::list_tools))
		// User MCP Servers
		.route("/api/v1/mcp-servers", get(public::mcp::list_servers))
		.route("/api/v1/mcp-servers", post(public::mcp::create_server))
		.route("/api/v1/mcp-servers/{id}", get(public::mcp::get_server))
		.route("/api/v1/mcp-servers/{id}", put(public::mcp::update_server))
		.route("/api/v1/mcp-servers/{id}", delete(public::mcp::delete_server))
		.route("/api/v1/mcp-servers/{id}/discover", post(public::mcp::discover_server))
		.route("/api/v1/mcp-servers/{id}/sync-tools", post(public::mcp::sync_tools_from_client))
		// Images CDN (public, no auth)
		.route("/api/v1/images/{id}", get(public::images::serve_image))
		.route("/api/v1/images", post(public::images::upload_image))
		.layer(CookieManagerLayer::new())
}
