//! Encryption utilities for API keys using AES-256-GCM
//!
//! Encrypts API keys when `ENCRYPTION_KEY` environment variable is set.
//! Falls back to plaintext storage when not configured.

use aes_gcm::{
	Aes256Gcm, Nonce,
	aead::{Aead, KeyInit},
};
use std::sync::OnceLock;

type EncryptionKey = [u8; 32];
const NONCE_SIZE: usize = 12;

static ENCRYPTION_KEY: OnceLock<Option<EncryptionKey>> = OnceLock::new();
static CIPHER: OnceLock<Option<Aes256Gcm>> = OnceLock::new();

/// Initialize the encryption key from environment variable.
/// Call this once at startup.
pub fn init() {
	let key_opt = std::env::var("ENCRYPTION_KEY").ok().and_then(|key| {
		if key.len() != 64 {
			eprintln!("[ENCRYPTION] ENCRYPTION_KEY must be 64 hex characters (32 bytes)");
			return None;
		}
		hex_decode(&key).ok()
	});

	let key_ref = ENCRYPTION_KEY.get_or_init(|| key_opt);
	CIPHER.get_or_init(|| key_ref.as_ref().map(|key| Aes256Gcm::new(key.as_ref().into())));
}

/// Check if encryption is enabled
#[must_use]
pub fn is_enabled() -> bool {
	CIPHER.get().and_then(|c| c.as_ref()).is_some()
}

/// Store an API key, encrypting if `ENCRYPTION_KEY` is set
#[must_use]
pub fn encrypt_api_key(plaintext: &str) -> String {
	match CIPHER.get().and_then(|c| c.as_ref()) {
		Some(cipher) => match encrypt(plaintext.as_bytes(), cipher) {
			Ok(ciphertext) => format!("enc:{}", hex_encode(&ciphertext)),
			Err(e) => {
				eprintln!("[ENCRYPTION] Failed to encrypt: {e}");
				plaintext.to_string()
			}
		},
		None => plaintext.to_string(),
	}
}

/// Retrieve an API key, decrypting if it was encrypted
#[must_use]
pub fn decrypt_api_key(stored: &str) -> String {
	if let Some(hex_data) = stored.strip_prefix("enc:") {
		match CIPHER.get().and_then(|c| c.as_ref()) {
			Some(cipher) => match hex_decode_to_vec(hex_data) {
				Ok(ciphertext) => match decrypt(&ciphertext, cipher) {
					Ok(plaintext) => String::from_utf8(plaintext).unwrap_or_else(|_| stored.to_string()),
					Err(e) => {
						eprintln!("[ENCRYPTION] Failed to decrypt: {e}");
						stored.to_string()
					}
				},
				Err(_) => stored.to_string(),
			},
			None => {
				eprintln!("[ENCRYPTION] Encrypted data but no ENCRYPTION_KEY set");
				stored.to_string()
			}
		}
	} else {
		stored.to_string()
	}
}

/// Encrypt plaintext using AES-256-GCM
fn encrypt(plaintext: &[u8], cipher: &Aes256Gcm) -> Result<Vec<u8>, &'static str> {
	let nonce = rand_bytes();
	let nonce_arr = Nonce::from_slice(&nonce);

	cipher
		.encrypt(nonce_arr, plaintext)
		.map(|mut ciphertext| {
			ciphertext.extend_from_slice(&nonce);
			ciphertext
		})
		.map_err(|_| "Encryption failed")
}

/// Decrypt ciphertext using AES-256-GCM
fn decrypt(ciphertext: &[u8], cipher: &Aes256Gcm) -> Result<Vec<u8>, &'static str> {
	if ciphertext.len() < NONCE_SIZE {
		return Err("Ciphertext too short");
	}

	let (ciphertext_part, nonce_part) = ciphertext.split_at(ciphertext.len() - NONCE_SIZE);
	let nonce = Nonce::from_slice(nonce_part);

	cipher.decrypt(nonce, ciphertext_part).map_err(|_| "Decryption failed - check key or data corruption")
}

fn hex_encode(data: &[u8]) -> String {
	data.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode_to_vec(hex: &str) -> Result<Vec<u8>, &'static str> {
	if hex.len() % 2 != 0 {
		return Err("Invalid hex length");
	}

	let mut result = Vec::with_capacity(hex.len() / 2);
	let mut chars = hex.chars();

	while let (Some(a), Some(b)) = (chars.next(), chars.next()) {
		let high = hex_char_to_u8(a)?;
		let low = hex_char_to_u8(b)?;
		result.push((high << 4) | low);
	}

	Ok(result)
}

fn hex_decode(hex: &str) -> Result<EncryptionKey, &'static str> {
	let vec = hex_decode_to_vec(hex)?;
	if vec.len() != 32 {
		return Err("Invalid key length");
	}

	let mut result = [0u8; 32];
	result.copy_from_slice(&vec);
	Ok(result)
}

fn hex_char_to_u8(c: char) -> Result<u8, &'static str> {
	match c {
		'0'..='9' => Ok(c as u8 - b'0'),
		'a'..='f' => Ok(c as u8 - b'a' + 10),
		'A'..='F' => Ok(c as u8 - b'A' + 10),
		_ => Err("Invalid hex character"),
	}
}

fn rand_bytes() -> [u8; NONCE_SIZE] {
	use rand::RngCore;
	let mut rng = rand::thread_rng();
	let mut nonce = [0u8; NONCE_SIZE];
	rng.fill_bytes(&mut nonce);
	nonce
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_encrypt_decrypt_roundtrip() {
		let key: EncryptionKey = [0x42u8; 32];
		let cipher = Aes256Gcm::new(key.as_ref().into());
		let plaintext = b"my-secret-api-key";

		let ciphertext = encrypt(plaintext, &cipher).unwrap();
		let decrypted = decrypt(&ciphertext, &cipher).unwrap();

		assert_eq!(plaintext.to_vec(), decrypted);
	}

	#[test]
	fn test_hex_encode_decode() {
		let original = [
			0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
			0x00, 0x11, 0x22, 0x33, 0x44, 0x55,
		];
		let hex = hex_encode(&original);
		let decoded = hex_decode_to_vec(&hex).unwrap();
		assert_eq!(original.to_vec(), decoded);
	}

	#[test]
	fn test_plaintext_passthrough() {
		let plaintext = "not-encrypted-key";
		let result = decrypt_api_key(plaintext);
		assert_eq!(plaintext, result);
	}

	#[test]
	fn test_encryption_key_decode() {
		let hex = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
		let decoded = hex_decode(hex).unwrap();

		let expected: EncryptionKey = [
			0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19,
			0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
		];

		let _cipher = Aes256Gcm::new(decoded.as_ref().into());
		assert_eq!(expected, decoded);
	}
}
