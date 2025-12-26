//! Base endpoint handler.
//!
//! Returns application base data including translations.

use crate::i18n::I18n;
use crate::types::BaseResponse;
use axum::Json;

/// GET /api/v1/base
///
/// Returns base application data including translations.
pub async fn get_base() -> Json<BaseResponse> {
	let i18n = I18n::get().all();

	Json(BaseResponse { i18n })
}
