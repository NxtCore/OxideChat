pub mod analytics;
pub mod budgets;
pub mod catalog;
pub mod config;
pub mod gateway;
pub mod i18n;
pub mod mcp;
pub mod models;
pub mod provider_billing;
pub mod providers;
pub mod teams;
pub mod tools;
pub mod users;

use crate::types::JobState;
use axum::{
	Router,
	routing::{delete, get, patch, post, put},
};
use std::sync::Arc;

pub fn router() -> Router<Arc<JobState>> {
	Router::new()
		// Admin i18n
		.route("/api/v1/admin/i18n", get(i18n::list_translations))
		.route("/api/v1/admin/i18n/translations", put(i18n::upsert_translation))
		.route("/api/v1/admin/i18n/translations/{id}", delete(i18n::delete_translation))
		// Admin AI Providers
		.route("/api/v1/admin/providers", get(providers::list_providers).post(providers::create_provider))
		.route("/api/v1/admin/providers/billing", get(provider_billing::list_billing))
		.route(
			"/api/v1/admin/providers/{id}/billing",
			get(provider_billing::get_billing)
				.put(provider_billing::update_billing)
				.delete(provider_billing::delete_billing),
		)
		.route("/api/v1/admin/providers/{id}/billing/refresh", post(provider_billing::refresh_billing))
		.route("/api/v1/admin/providers/test", post(providers::test_provider_inline))
		.route("/api/v1/admin/providers/{id}", get(providers::get_provider).put(providers::update_provider).delete(providers::delete_provider))
		.route("/api/v1/admin/providers/{id}/test", post(providers::test_provider))
		.route("/api/v1/admin/providers/{id}/sync", post(providers::sync_provider))
		.route("/api/v1/admin/providers/{id}/models", get(providers::list_models))
		.route("/api/v1/admin/providers/{id}/catalog", get(catalog::list_catalog))
		// Admin Tools
		.route("/api/v1/admin/tools", get(tools::list_tools).post(tools::create_tool))
		.route("/api/v1/admin/tools/wasm/upload", post(tools::upload_wasm))
		.route("/api/v1/admin/tools/{id}", get(tools::get_tool).put(tools::update_tool).delete(tools::delete_tool))
		.route("/api/v1/admin/tools/{id}/settings", get(tools::get_tool_settings).put(tools::set_tool_settings))
		.route("/api/v1/admin/tools/{id}/test", post(tools::test_tool))
		// Admin MCP Servers
		.route("/api/v1/admin/mcp-servers", get(mcp::list_servers).post(mcp::create_server))
		.route("/api/v1/admin/mcp-servers/{id}", get(mcp::get_server).put(mcp::update_server).delete(mcp::delete_server))
		.route("/api/v1/admin/mcp-servers/{id}/discover", post(mcp::discover_server))
		.route("/api/v1/admin/mcp-servers/{id}/health-check", post(mcp::health_check))
		// Admin Teams
		.route("/api/v1/admin/teams", get(teams::list_teams).post(teams::create_team))
		.route("/api/v1/admin/teams/{id}", get(teams::get_team).put(teams::save_team).patch(teams::update_team).delete(teams::delete_team))
		.route("/api/v1/admin/teams/{id}/members", put(teams::set_team_members))
		.route("/api/v1/admin/teams/{id}/members/{user_id}/role", put(teams::set_team_member_role))
		.route("/api/v1/admin/teams/{id}/models", put(teams::set_team_models))
		.route("/api/v1/admin/teams/{id}/budget", patch(teams::update_team_budget))
		// Admin Budgets
		.route("/api/v1/admin/budgets", get(budgets::list_budgets).post(budgets::create_budget))
		.route("/api/v1/admin/budgets/overview/users", get(budgets::user_overview))
		.route("/api/v1/admin/budgets/overview/teams", get(budgets::team_overview))
		.route("/api/v1/admin/budgets/resets", get(budgets::reset_history).post(budgets::reset_budget))
		.route("/api/v1/admin/budgets/{id}", patch(budgets::update_budget).delete(budgets::delete_budget))
		.route(
			"/api/v1/admin/budgets/{id}/assignments",
			get(budgets::get_budget_assignments).post(budgets::assign_budget),
		)
		.route("/api/v1/admin/budgets/{id}/assignments/{assignment_id}", delete(budgets::delete_assignment))
		.route("/api/v1/admin/analytics", get(analytics::get_analytics))
		.route("/api/v1/admin/gateway/keys", get(gateway::list_keys))
		.route("/api/v1/admin/gateway/keys/{id}", get(gateway::get_key))
		.route("/api/v1/admin/gateway/keys/{id}/requests", get(gateway::list_key_requests))
		.route("/api/v1/admin/gateway/keys/{id}/revoke", post(gateway::revoke_key))
		.route("/api/v1/admin/gateway/keys/{id}/disable", post(gateway::disable_key))
		.route("/api/v1/admin/gateway/keys/{id}/enable", post(gateway::enable_key))
		.route("/api/v1/admin/gateway/requests", get(gateway::list_requests))
		.route("/api/v1/admin/gateway/teams", get(gateway::list_teams))
		.route("/api/v1/admin/gateway/teams/{id}", get(gateway::get_team))
		.route("/api/v1/admin/gateway/teams/{id}/limits", put(gateway::set_team_limits))
		.route("/api/v1/admin/gateway/teams/{id}/access", put(gateway::set_team_access))
		.route("/api/v1/admin/gateway/audit", get(gateway::list_audit))
		.route("/api/v1/admin/gateway/events", get(gateway::list_events))
		// Admin Users
		.route("/api/v1/admin/users", get(users::list_users).post(users::create_user))
		.route("/api/v1/admin/users/{id}", get(users::get_user).put(users::update_user).delete(users::delete_user))
		.route("/api/v1/admin/users/{id}/roles", put(users::set_user_roles))
		.route("/api/v1/admin/users/{id}/teams", put(users::set_user_teams))
		.route("/api/v1/admin/users/{id}/password", put(users::reset_password))
		// Admin Models
		.route("/api/v1/admin/models", get(models::list_models))
		.route("/api/v1/admin/image-models", get(models::list_image_models))
		.route("/api/v1/admin/image-model-providers", get(models::list_image_model_providers))
		.route("/api/v1/admin/models/{id}", get(models::get_model).patch(models::patch_model))
		.route("/api/v1/admin/models/{id}/pricing", get(models::get_model_pricing).put(models::put_model_pricing).delete(models::delete_model_pricing))
		.route("/api/v1/admin/models/{id}/provider-options", get(catalog::list_provider_options))
		.route("/api/v1/admin/models/{id}/provider-options/refresh", post(catalog::refresh_provider_options))
		// Admin Config
		.route("/api/v1/admin/config", get(config::get_admin_config).patch(config::update_global_config))
		.route("/api/v1/config", get(config::get_global_config))
}
