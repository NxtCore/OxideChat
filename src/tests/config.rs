#[cfg(test)]
mod tests {
	use crate::config::ConfigValues;
	use crate::i18n::Language;
	use crate::types::UpdateGlobalConfigRequest;

	#[test]
	fn test_config_values_default() {
		let values = ConfigValues::default();
		assert_eq!(values.language, Language::En);
	}

	#[test]
	fn update_request_reads_language_and_rejects_unknown_ones() {
		let request: UpdateGlobalConfigRequest = serde_json::from_str(r#"{"language":"de"}"#).unwrap();
		assert_eq!(request.language, Some(Language::De));
		assert!(serde_json::from_str::<UpdateGlobalConfigRequest>(r#"{"language":"fr"}"#).is_err());
	}

	#[test]
	fn branding_update_tells_omitted_from_null() {
		let request: UpdateGlobalConfigRequest = serde_json::from_str(r#"{"instance":{"name":"Acme Chat","icon_id":null}}"#).unwrap();
		let instance = request.instance.unwrap();
		assert_eq!(instance.name, Some(Some("Acme Chat".to_string())));
		assert_eq!(instance.icon_id, Some(None));

		let request: UpdateGlobalConfigRequest = serde_json::from_str(r#"{"instance":{}}"#).unwrap();
		let instance = request.instance.unwrap();
		assert_eq!(instance.name, None);
		assert_eq!(instance.icon_id, None);
		assert!(serde_json::from_str::<UpdateGlobalConfigRequest>("{}").unwrap().instance.is_none());
	}

	#[test]
	fn title_generation_update_is_read_whole() {
		let request: UpdateGlobalConfigRequest = serde_json::from_str(r#"{"title_generation":{"enabled":true,"model_id":null}}"#).unwrap();
		let title = request.title_generation.unwrap();
		assert!(title.enabled);
		assert_eq!(title.model_id, None);
		assert!(serde_json::from_str::<UpdateGlobalConfigRequest>("{}").unwrap().title_generation.is_none());
		assert!(!ConfigValues::default().title_generation.enabled);
	}

	mod branding_icon {
		use crate::utils::images::{is_safe_stored_image, store_image};
		use sqlx::PgPool;
		use uuid::Uuid;

		const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

		#[sqlx::test]
		async fn only_stored_images_of_an_allowed_type_qualify(pool: PgPool) -> Result<(), String> {
			let png = store_image(&pool, PNG, "image/png", None, Some("upload")).await?;
			assert!(is_safe_stored_image(&pool, png.id).await?);

			let svg = store_image(&pool, b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script/></svg>", "image/png", None, Some("upload")).await?;
			assert!(!is_safe_stored_image(&pool, svg.id).await?);

			assert!(!is_safe_stored_image(&pool, Uuid::new_v4()).await?);
			Ok(())
		}
	}
}
