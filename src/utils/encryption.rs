//! Optional encryption utilities for API keys
//!
//! Encrypts API keys when `ENCRYPTION_KEY` environment variable is set.
//! Falls back to plaintext storage when not configured.

use std::sync::OnceLock;

static ENCRYPTION_KEY: OnceLock<Option<[u8; 32]>> = OnceLock::new();

/// Initialize the encryption key from environment variable.
/// Call this once at startup.
pub fn init() {
	ENCRYPTION_KEY.get_or_init(|| {
		std::env::var("ENCRYPTION_KEY").ok().and_then(|key| {
			// Expect a 64-character hex string (32 bytes)
			if key.len() != 64 {
				eprintln!("[ENCRYPTION] ENCRYPTION_KEY must be 64 hex characters (32 bytes)");
				return None;
			}
			hex_decode(&key).ok()
		})
	});
}

/// Check if encryption is enabled
#[must_use]
pub fn is_enabled() -> bool {
	ENCRYPTION_KEY.get().and_then(|k| k.as_ref()).is_some()
}

/// Store an API key, encrypting if `ENCRYPTION_KEY` is set
#[must_use]
pub fn encrypt_api_key(plaintext: &str) -> String {
	match ENCRYPTION_KEY.get().and_then(|k| k.as_ref()) {
		Some(key) => match encrypt(plaintext.as_bytes(), key) {
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
		match ENCRYPTION_KEY.get().and_then(|k| k.as_ref()) {
			Some(key) => match hex_decode(hex_data) {
				Ok(ciphertext) => match decrypt(&ciphertext, key) {
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

/// Simple XOR-based encryption with a random nonce (for demonstration).
/// In production, consider using a proper crypto library like `ring` or `aes-gcm`.
fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, &'static str> {
	// Generate a random 16-byte nonce
	let nonce: [u8; 16] = rand_bytes();

	// XOR encrypt (simplified - use AES-GCM in production)
	let mut ciphertext = Vec::with_capacity(16 + plaintext.len());
	ciphertext.extend_from_slice(&nonce);

	for (i, byte) in plaintext.iter().enumerate() {
		let key_byte = key[i % 32];
		let nonce_byte = nonce[i % 16];
		ciphertext.push(byte ^ key_byte ^ nonce_byte);
	}

	Ok(ciphertext)
}

fn decrypt(ciphertext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, &'static str> {
	if ciphertext.len() < 16 {
		return Err("Ciphertext too short");
	}

	let nonce = &ciphertext[..16];
	let encrypted = &ciphertext[16..];

	let mut plaintext = Vec::with_capacity(encrypted.len());
	for (i, byte) in encrypted.iter().enumerate() {
		let key_byte = key[i % 32];
		let nonce_byte = nonce[i % 16];
		plaintext.push(byte ^ key_byte ^ nonce_byte);
	}

	Ok(plaintext)
}

fn hex_encode(data: &[u8]) -> String {
	data.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(hex: &str) -> Result<[u8; 32], &'static str> {
	if hex.len() != 64 {
		return Err("Invalid hex length");
	}

	let mut result = [0u8; 32];
	for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
		let high = hex_char_to_u8(chunk[0])?;
		let low = hex_char_to_u8(chunk[1])?;
		result[i] = (high << 4) | low;
	}

	Ok(result)
}

fn hex_char_to_u8(c: u8) -> Result<u8, &'static str> {
	match c {
		b'0'..=b'9' => Ok(c - b'0'),
		b'a'..=b'f' => Ok(c - b'a' + 10),
		b'A'..=b'F' => Ok(c - b'A' + 10),
		_ => Err("Invalid hex character"),
	}
}

fn rand_bytes() -> [u8; 16] {
	use std::time::{SystemTime, UNIX_EPOCH};
	let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();

	let mut result = [0u8; 16];
	let mut state = seed;
	for byte in &mut result {
		state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
		*byte = (state >> 33) as u8;
	}
	result
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_encrypt_decrypt_roundtrip() {
		let key = [0x42u8; 32];
		let plaintext = b"my-secret-api-key";

		let ciphertext = encrypt(plaintext, &key).unwrap();
		let decrypted = decrypt(&ciphertext, &key).unwrap();

		assert_eq!(plaintext.to_vec(), decrypted);
	}

	#[test]
	fn test_hex_encode_decode() {
		let original = [
			0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
			0x00, 0x11, 0x22, 0x33, 0x44, 0x55,
		];
		let hex = hex_encode(&original);
		let decoded = hex_decode(&hex).unwrap();
		assert_eq!(original, decoded);
	}

	#[test]
	fn test_plaintext_passthrough() {
		let plaintext = "not-encrypted-key";
		let result = decrypt_api_key(plaintext);
		assert_eq!(plaintext, result);
	}
}
