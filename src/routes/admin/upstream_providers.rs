//! Admin routes for the upstream providers routing providers can send requests to.

use crate::types::consts::{ADMIN_CONFIG_EDIT, ADMIN_CONFIG_VIEW, ADMIN_TEAMS_VIEW};
use crate::types::{AppError, AuthUser, JobState, UpstreamGatewayResponse, json_ok};
use axum::{extract::State, response::Response};
use std::sync::Arc;

/// GET /api/v1/admin/upstream-providers
///
/// The routing provider kinds the installation uses, each with the upstream providers it can send requests to,
/// offered when editing a provider allowlist on the installation settings or on a team.
pub async fn list_upstream_providers(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	if !auth.check_permission(&state.db, ADMIN_CONFIG_VIEW).await? {
		auth.require(&state.db, ADMIN_TEAMS_VIEW).await?;
	}
	json_ok(UpstreamGatewayResponse::load_all(&state.db).await?)
}

/// POST /api/v1/admin/upstream-providers/sync
///
/// Syncs the upstream providers of every routing provider kind from the routing providers themselves and
/// returns the new lists.
pub async fn sync_upstream_providers(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_CONFIG_EDIT).await?;
	json_ok(UpstreamGatewayResponse::sync_all(&state.db).await?)
}
