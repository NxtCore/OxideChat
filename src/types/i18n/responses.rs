use super::Translation;

#[derive(Debug, serde::Serialize)]
pub struct TranslationsResponse {
	pub translations: Vec<Translation>,
}
