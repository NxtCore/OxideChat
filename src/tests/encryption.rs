#[cfg(test)]
mod credential_encryption {
	use crate::utils::encryption::{EncryptionError, decrypt_api_key, encrypt_api_key, init_for_test};
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
}
