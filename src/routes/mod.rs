//! Route handlers for OxideChat API.

pub mod admin;
pub mod base;

use crate::AppState;
use axum::{
	Router,
	routing::{delete, get, put},
};
use std::sync::Arc;

/// Build the API router with all routes.
pub fn build_router() -> Router<Arc<AppState>> {
	let admin_i18n_routes = Router::new()
		.route("/", get(admin::i18n::list_translations))
		.route("/translations", put(admin::i18n::upsert_translation))
		.route("/translations/{id}", delete(admin::i18n::delete_translation));

	let admin_routes = Router::new().nest("/i18n", admin_i18n_routes);

	let v1_routes = Router::new().route("/base", get(base::get_base)).nest("/admin", admin_routes);

	Router::new().nest("/api/v1", v1_routes)
}
