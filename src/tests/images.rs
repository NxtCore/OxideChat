#[cfg(test)]
mod tests {
	use crate::utils::images::{parse_data_uri, safe_image_mime};
	use base64::Engine as _;

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
}
