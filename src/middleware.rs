use crate::routes::public::auth::get_current_user;
use crate::types::{JobState, RequestContext};
use axum::{extract::State, middleware::Next, response::Response};
use std::sync::Arc;
use tower_cookies::Cookies;

/// Loads the session user once and makes it available to downstream handlers.
pub async fn load_current_user(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	mut request: axum::extract::Request,
	next: Next,
) -> Response {
	let user = get_current_user(&state.db, &cookies).await;
	request.extensions_mut().insert(RequestContext { user });
	next.run(request).await
}
