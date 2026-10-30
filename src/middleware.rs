use crate::routes::public::auth::get_current_user;
use crate::types::{JobState, RequestContext};
use axum::{
	extract::State,
	http::{HeaderName, HeaderValue, Method, header},
	middleware::Next,
	response::Response,
};
use std::sync::Arc;
use tower_cookies::Cookies;
use tower_http::cors::{AllowOrigin, CorsLayer};

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

/// Attaches baseline security headers to every response.
pub async fn security_headers(request: axum::extract::Request, next: Next) -> Response {
	let mut response = next.run(request).await;
	let headers = response.headers_mut();
	headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
	headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
	headers.insert(
		HeaderName::from_static("referrer-policy"),
		HeaderValue::from_static("strict-origin-when-cross-origin"),
	);
	headers.insert(
		HeaderName::from_static("content-security-policy"),
		HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
	);
	response
}

/// Builds the CORS layer, allowing credentialed cross-origin requests only from `BASE_URL`.
///
/// Falls back to an empty origin list (same-origin only, no CORS headers emitted) when
/// `BASE_URL` is unset or cannot be parsed into scheme and host.
pub fn cors_layer() -> CorsLayer {
	let allowed_origin = std::env::var("BASE_URL").ok().and_then(|base_url| {
		let uri = base_url.parse::<axum::http::Uri>().ok()?;
		let scheme = uri.scheme_str()?;
		let authority = uri.authority()?;
		HeaderValue::from_str(&format!("{scheme}://{authority}")).ok()
	});
	let allow_origin = match allowed_origin {
		Some(origin) => AllowOrigin::list([origin]),
		None => AllowOrigin::list(Vec::new()),
	};
	CorsLayer::new()
		.allow_origin(allow_origin)
		.allow_credentials(true)
		.allow_methods([
			Method::GET,
			Method::POST,
			Method::PUT,
			Method::PATCH,
			Method::DELETE,
			Method::OPTIONS,
		])
		.allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
		.expose_headers([
			HeaderName::from_static("x-ratelimit-limit-requests"),
			HeaderName::from_static("x-ratelimit-remaining-requests"),
			HeaderName::from_static("x-ratelimit-reset-requests"),
			HeaderName::from_static("x-ratelimit-limit-tokens"),
			HeaderName::from_static("x-ratelimit-remaining-tokens"),
			HeaderName::from_static("x-ratelimit-reset-tokens"),
			HeaderName::from_static("retry-after"),
			HeaderName::from_static("x-oxide-concurrent-limit"),
			HeaderName::from_static("x-oxide-concurrent-remaining"),
		])
}
