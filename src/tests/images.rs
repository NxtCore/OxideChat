#[cfg(test)]
mod tests {
	use crate::utils::images::{get_image, parse_data_uri, safe_image_mime, store_from_data_uri};
	use base64::Engine as _;
	use sqlx::PgPool;
	use uuid::Uuid;

	const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";

	#[test]
	fn data_uri_accepts_matching_safe_raster_image() {
		let data_uri = format!("data:image/png;base64,{PNG_1X1}");
		let (mime_type, data) = parse_data_uri(&data_uri).unwrap();

		assert_eq!(mime_type, "image/png");
		assert_eq!(safe_image_mime(&data, &mime_type), Some("image/png"));
	}

	#[test]
	fn data_uri_rejects_active_content_types() {
		let html = base64::engine::general_purpose::STANDARD.encode(b"<script>alert(1)</script>");
		let svg = base64::engine::general_purpose::STANDARD.encode(br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#);

		assert!(parse_data_uri(&format!("data:text/html;base64,{html}")).is_err());
		assert!(parse_data_uri(&format!("data:image/svg+xml;base64,{svg}")).is_err());
	}

	#[test]
	fn data_uri_rejects_declared_mime_mismatch() {
		let data_uri = format!("data:image/jpeg;base64,{PNG_1X1}");

		assert!(parse_data_uri(&data_uri).is_err());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn store_and_get_image_roundtrip(pool: PgPool) {
		let data_uri = format!("data:image/png;base64,{PNG_1X1}");
		let stored = store_from_data_uri(&pool, &data_uri, None, Some("test")).await.unwrap();

		let (data, mime_type) = get_image(&pool, stored.id).await.unwrap().unwrap();
		assert_eq!(mime_type, "image/png");
		assert_eq!(data, base64::engine::general_purpose::STANDARD.decode(PNG_1X1).unwrap());

		assert!(get_image(&pool, Uuid::new_v4()).await.unwrap().is_none());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn store_from_data_uri_rejects_invalid_payloads_without_storing(pool: PgPool) {
		assert!(store_from_data_uri(&pool, "not-a-data-uri", None, None).await.is_err());
		assert!(store_from_data_uri(&pool, "data:image/png;base64,!!!not-base64!!!", None, None).await.is_err());

		let html = base64::engine::general_purpose::STANDARD.encode(b"<script>alert(1)</script>");
		assert!(store_from_data_uri(&pool, &format!("data:text/html;base64,{html}"), None, None).await.is_err());

		let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM images")
			.fetch_one(&pool)
			.await
			.unwrap();
		assert_eq!(count, 0);
	}
}
