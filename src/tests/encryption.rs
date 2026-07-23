#[cfg(test)]
mod credential_encryption {
	use crate::utils::encryption::{
		EncryptionError, SECRET_MASK, decrypt_api_key, decrypt_object_values, decrypt_schema_secrets, encrypt_api_key, encrypt_object_values, encrypt_schema_secrets,
		init_for_test, mask_object_values, mask_schema_secrets,
	};
	use aes_gcm::{
		Aes256Gcm, Nonce,
		aead::{Aead, KeyInit},
	};
	use std::fmt::Write;

	#[test]
	fn encrypts_and_authenticates_credentials() -> Result<(), EncryptionError> {
		init_for_test([0x42; 32])?;
		let encrypted = encrypt_api_key("provider-secret")?;
		assert!(encrypted.starts_with("oxide:v1:"));
		assert_eq!(decrypt_api_key(&encrypted)?, "provider-secret");
		assert_eq!(decrypt_api_key("existing-plaintext")?, "existing-plaintext");
		let nonce = [0x24; 12];
		let cipher = Aes256Gcm::new([0x42; 32].as_ref().into());
		let mut legacy = cipher
			.encrypt(Nonce::from_slice(&nonce), b"legacy-secret".as_ref())
			.map_err(|_| EncryptionError::EncryptionFailed)?;
		legacy.extend_from_slice(&nonce);
		let legacy = legacy.iter().fold(String::from("enc:"), |mut encoded, byte| {
			let _ = write!(encoded, "{byte:02x}");
			encoded
		});
		assert_eq!(decrypt_api_key(&legacy)?, "legacy-secret");

		let mut tampered = encrypted.into_bytes();
		let last = tampered.len() - 1;
		tampered[last] = if tampered[last] == b'A' { b'B' } else { b'A' };
		let tampered = String::from_utf8(tampered).map_err(|_| EncryptionError::InvalidFormat)?;
		assert!(decrypt_api_key(&tampered).is_err());
		Ok(())
	}

	#[test]
	fn protects_structured_secrets_and_preserves_masks() -> Result<(), EncryptionError> {
		init_for_test([0x42; 32])?;
		let schema = serde_json::json!({
			"type": "object",
			"properties": {
				"api_key": {"type": "string", "secret": true},
				"provider": {"type": "string"}
			}
		});
		let settings = serde_json::json!({"api_key": "secret", "provider": "tavily"});
		let protected = encrypt_schema_secrets(&settings, &schema, None)?;
		assert!(protected["api_key"].as_str().is_some_and(|value| value.starts_with("oxide:v1:")));
		assert_eq!(protected["provider"], "tavily");
		assert_eq!(decrypt_schema_secrets(&protected, &schema)?, settings);
		assert_eq!(mask_schema_secrets(&protected, &schema)?["api_key"], SECRET_MASK);
		let update = serde_json::json!({"api_key": SECRET_MASK, "provider": "exa"});
		let updated = encrypt_schema_secrets(&update, &schema, Some(&protected))?;
		assert_eq!(updated["api_key"], protected["api_key"]);

		let headers = serde_json::json!({"Authorization": "Bearer secret"});
		let protected_headers = encrypt_object_values(&headers, None)?;
		assert_eq!(decrypt_object_values(&protected_headers)?, headers);
		assert_eq!(mask_object_values(&protected_headers)["Authorization"], SECRET_MASK);
		Ok(())
	}
}
