use super::Translation;

#[derive(Debug, serde::Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TranslationsResponse {
	pub translations: Vec<Translation>,
}
