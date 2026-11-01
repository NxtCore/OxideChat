pub mod auth;
pub mod base;
pub mod chats;
pub mod developer;
pub mod images;
pub mod mcp;
pub mod messages;
pub mod models;
pub mod oauth;
pub mod openai;
pub mod preferences;
pub mod providers;
pub mod streaming;
pub mod tools;
pub mod users;
pub mod workspaces;

use crate::types::JobState;
use axum::{
	Router,
	routing::{delete, get, patch, post},
};
use std::sync::Arc;

pub fn router() -> Router<Arc<JobState>> {
	Router::new()
		.route("/api/v1/health", get(base::health))
		.route("/api/v1/base", get(base::get_base))
		.route("/api/v1/users/@me", get(users::get_me))
		.route("/api/v1/users/@me/sessions", get(users::list_my_sessions))
		.route("/api/v1/users/@me/sessions/{id}", delete(users::revoke_my_session))
		.route("/api/v1/me/budget", get(users::get_my_budget))
		.route("/api/v1/me/analytics", get(users::get_my_analytics))
		.route("/api/v1/users/@me/preferences", get(preferences::get_preferences).patch(preferences::update_preferences))
		// Workspaces
		.route("/api/v1/workspaces", get(workspaces::list_workspaces).post(workspaces::create_workspace))
		.route("/api/v1/workspaces/{id}", get(workspaces::get_workspace).patch(workspaces::update_workspace).delete(workspaces::delete_workspace))
		// Chats
		.route("/api/v1/chats", get(chats::list_chats).post(chats::create_chat))
		.route("/api/v1/chats/{id}", get(chats::get_chat).patch(chats::update_chat).delete(chats::delete_chat))
		// Messages
		.route("/api/v1/chats/{chat_id}/messages", get(messages::list_messages).post(messages::send_message))
		.route("/api/v1/chats/{chat_id}/stream", post(streaming::stream_completion))
		.route("/api/v1/chats/{chat_id}/stream/tool-result", post(streaming::submit_client_tool_result))
		// Message Forks
		.route("/api/v1/chats/{chat_id}/messages/{message_id}/edit", post(messages::edit_message))
		.route("/api/v1/chats/{chat_id}/messages/{message_id}/switch-fork", post(messages::switch_fork))
		.route("/api/v1/chats/{chat_id}/messages/{message_id}/siblings", get(messages::get_siblings))
		.route("/api/v1/chats/{chat_id}/messages/{message_id}/fork", delete(messages::delete_fork))
		.route("/api/v1/chats/{chat_id}/messages/{message_id}/branch", post(messages::branch_from_message))
		// Models
		.route("/api/v1/models", get(models::list_models))
		.route("/api/v1/models/{id}/favorite", post(models::set_model_favorite))
		.route("/api/v1/models/{id}/provider-options", get(models::get_provider_options))
		// Providers
		.route("/api/v1/providers", get(providers::list_providers))
		// Tools
		.route("/api/v1/tools", get(tools::list_tools))
		// User MCP Servers
		.route("/api/v1/mcp-servers", get(mcp::list_servers).post(mcp::create_server))
		.route("/api/v1/mcp-servers/{id}", get(mcp::get_server).put(mcp::update_server).delete(mcp::delete_server))
		.route("/api/v1/mcp-servers/{id}/discover", post(mcp::discover_server))
		.route("/api/v1/mcp-servers/{id}/sync-tools", post(mcp::sync_tools_from_client))
		// Developer Gateway Keys
		.route("/api/v1/developer/teams", get(developer::list_teams))
		.route("/api/v1/developer/models", get(developer::list_models))
		.route("/api/v1/developer/team-keys", get(developer::list_team_keys))
		.route("/api/v1/developer/keys", get(developer::list_keys).post(developer::create_key))
		.route("/api/v1/developer/keys/{id}", patch(developer::update_key))
		.route("/api/v1/developer/keys/{id}/revoke", post(developer::revoke_key))
		.route("/api/v1/developer/keys/{id}/rotate", post(developer::rotate_key))
		.route(
			"/api/v1/developer/keys/{id}/limits",
			get(developer::get_limits).put(developer::set_limits).delete(developer::clear_limits),
		)
		.route(
			"/api/v1/developer/keys/{id}/budget",
			get(developer::get_budget).put(developer::set_budget).delete(developer::clear_budget),
		)
		// Developer Request Logs
		.route("/api/v1/developer/request-logs", get(developer::list_request_logs))
		.route("/api/v1/developer/request-logs/export", get(developer::export_request_logs))
		.route(
			"/api/v1/developer/request-logs/retention",
			get(developer::get_request_log_retention).put(developer::set_request_log_retention),
		)
		// Images (session-authenticated)
		.route("/api/v1/images/{id}", get(images::serve_image))
		.route("/api/v1/images", post(images::upload_image))
}
