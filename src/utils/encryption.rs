//! Encryption utilities for credentials using AES-256-GCM.

use aes_gcm::{
	Aes256Gcm, Nonce,
	aead::{Aead, KeyInit, Payload},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use std::{
	fs::{self, OpenOptions},
	io::{ErrorKind, Write},
	path::{Path, PathBuf},
	sync::OnceLock,
};
use thiserror::Error;

use serde_json::Value;

type EncryptionKey = [u8; 32];
const NONCE_SIZE: usize = 12;
const CURRENT_PREFIX: &str = "oxide:v1:";
const LEGACY_PREFIX: &str = "enc:";
const ASSOCIATED_DATA: &[u8] = b"oxidechat:credential:v1";
const DEFAULT_DATA_DIR: &str = ".data";
const KEY_FILE_NAME: &str = "master.key";
pub(crate) const SECRET_MASK: &str = "***";

static CIPHER: OnceLock<CipherState> = OnceLock::new();

struct CipherState {
	cipher: Aes256Gcm,
	key_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
	Environment,
	File,
	Generated,
}

impl KeySource {
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Environment => "environment",
			Self::File => "local key file",
			Self::Generated => "new local key file",
		}
	}
}

#[derive(Debug, Error)]
pub enum EncryptionError {
	#[error("encryption has not been initialized")]
	NotInitialized,
	#[error("ENCRYPTION_KEY must contain exactly 64 hexadecimal characters")]
	InvalidEnvironmentKey,
	#[error("the local encryption key is invalid")]
	InvalidKeyFile,
	#[error("failed to access the local encryption key: {0}")]
	KeyFile(#[from] std::io::Error),
	#[error("encryption is already initialized")]
	AlreadyInitialized,
	#[error("the encrypted value has an unsupported format")]
	InvalidFormat,
	#[error("the encrypted value belongs to a different key")]
	KeyMismatch,
	#[error("credential encryption failed")]
	EncryptionFailed,
	#[error("credential decryption failed")]
	DecryptionFailed,
	#[error("the decrypted credential is not valid UTF-8")]
	InvalidPlaintext,
	#[error("a masked secret has no existing value")]
	MissingMaskedValue,
	#[error("secret values must be strings")]
	InvalidSecretValue,
}

/// Initialize credential encryption from the environment or a persistent local key.
///
/// # Errors
///
/// Returns an error when the configured key is invalid or the local key cannot be
/// securely loaded or created.
pub fn init() -> Result<KeySource, EncryptionError> {
	let (key, source) = match std::env::var("ENCRYPTION_KEY") {
		Ok(value) if !value.is_empty() => (decode_key(&value).map_err(|_| EncryptionError::InvalidEnvironmentKey)?, KeySource::Environment),
		Ok(_) | Err(std::env::VarError::NotPresent) => load_or_create_local_key()?,
		Err(std::env::VarError::NotUnicode(_)) => return Err(EncryptionError::InvalidEnvironmentKey),
	};
	set_key(&key)?;
	Ok(source)
}

fn set_key(key: &EncryptionKey) -> Result<(), EncryptionError> {
	let key_id = key_id(key);
	let state = CipherState {
		cipher: Aes256Gcm::new(key.as_ref().into()),
		key_id,
	};
	CIPHER.set(state).map_err(|_| EncryptionError::AlreadyInitialized)?;
	Ok(())
}

#[cfg(test)]
pub(crate) fn init_for_test(key: EncryptionKey) -> Result<(), EncryptionError> {
	if CIPHER.get().is_some() {
		return Ok(());
	}
	set_key(&key)
}

/// Check whether encryption was initialized successfully.
#[must_use]
pub fn is_enabled() -> bool {
	CIPHER.get().is_some()
}

/// Encrypt a credential using the active key.
///
/// # Errors
///
/// Returns an error when encryption is not initialized or encryption fails.
pub fn encrypt_api_key(plaintext: &str) -> Result<String, EncryptionError> {
	let state = CIPHER.get().ok_or(EncryptionError::NotInitialized)?;
	let nonce = random_nonce();
	let ciphertext = state
		.cipher
		.encrypt(
			Nonce::from_slice(&nonce),
			Payload {
				msg: plaintext.as_bytes(),
				aad: ASSOCIATED_DATA,
			},
		)
		.map_err(|_| EncryptionError::EncryptionFailed)?;
	let mut envelope = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
	envelope.extend_from_slice(&nonce);
	envelope.extend_from_slice(&ciphertext);
	Ok(format!("{CURRENT_PREFIX}{}:{}", state.key_id, BASE64.encode(envelope)))
}

/// Decrypt a current or legacy credential.
///
/// Plaintext values are returned unchanged to support existing installations until
/// those records are saved again.
///
/// # Errors
///
/// Returns an error for malformed values, key mismatches, or failed authentication.
pub fn decrypt_api_key(stored: &str) -> Result<String, EncryptionError> {
	let state = CIPHER.get().ok_or(EncryptionError::NotInitialized)?;
	if let Some(value) = stored.strip_prefix(CURRENT_PREFIX) {
		return decrypt_current(value, state);
	}
	if let Some(value) = stored.strip_prefix(LEGACY_PREFIX) {
		return decrypt_legacy(value, state);
	}
	Ok(stored.to_owned())
}

pub(crate) fn encrypt_secret_value(value: &str, existing: Option<&str>) -> Result<String, EncryptionError> {
	if value.starts_with(CURRENT_PREFIX) || value.starts_with(LEGACY_PREFIX) {
		return Ok(value.to_owned());
	}
	if value == SECRET_MASK {
		let existing = existing.ok_or(EncryptionError::MissingMaskedValue)?;
		return if existing.starts_with(CURRENT_PREFIX) || existing.starts_with(LEGACY_PREFIX) {
			Ok(existing.to_owned())
		} else {
			encrypt_api_key(existing)
		};
	}
	encrypt_api_key(value)
}

pub(crate) fn encrypt_object_values(value: &Value, existing: Option<&Value>) -> Result<Value, EncryptionError> {
	let mut protected = value.clone();
	let Some(object) = protected.as_object_mut() else {
		return Ok(protected);
	};
	let existing = existing.and_then(Value::as_object);
	for (key, value) in object {
		let secret = value.as_str().ok_or(EncryptionError::InvalidSecretValue)?;
		let previous = existing.and_then(|values| values.get(key)).and_then(Value::as_str);
		*value = Value::String(encrypt_secret_value(secret, previous)?);
	}
	Ok(protected)
}

pub(crate) fn decrypt_object_values(value: &Value) -> Result<Value, EncryptionError> {
	let mut revealed = value.clone();
	let Some(object) = revealed.as_object_mut() else {
		return Ok(revealed);
	};
	for value in object.values_mut() {
		let secret = value.as_str().ok_or(EncryptionError::InvalidSecretValue)?;
		*value = Value::String(decrypt_api_key(secret)?);
	}
	Ok(revealed)
}

pub(crate) fn mask_object_values(value: &Value) -> Value {
	let mut masked = value.clone();
	if let Some(object) = masked.as_object_mut() {
		for value in object.values_mut() {
			*value = Value::String(SECRET_MASK.to_owned());
		}
	}
	masked
}

pub(crate) fn encrypt_schema_secrets(value: &Value, schema: &Value, existing: Option<&Value>) -> Result<Value, EncryptionError> {
	transform_schema_secrets(value, schema, existing, SecretTransform::Encrypt)
}

pub(crate) fn decrypt_schema_secrets(value: &Value, schema: &Value) -> Result<Value, EncryptionError> {
	transform_schema_secrets(value, schema, None, SecretTransform::Decrypt)
}

pub(crate) fn mask_schema_secrets(value: &Value, schema: &Value) -> Result<Value, EncryptionError> {
	transform_schema_secrets(value, schema, None, SecretTransform::Mask)
}

#[derive(Clone, Copy)]
enum SecretTransform {
	Encrypt,
	Decrypt,
	Mask,
}

fn transform_schema_secrets(value: &Value, schema: &Value, existing: Option<&Value>, transform: SecretTransform) -> Result<Value, EncryptionError> {
	if schema.get("secret").and_then(Value::as_bool) == Some(true) {
		let secret = value.as_str().ok_or(EncryptionError::InvalidSecretValue)?;
		let transformed = match transform {
			SecretTransform::Encrypt => encrypt_secret_value(secret, existing.and_then(Value::as_str))?,
			SecretTransform::Decrypt => decrypt_api_key(secret)?,
			SecretTransform::Mask => SECRET_MASK.to_owned(),
		};
		return Ok(Value::String(transformed));
	}

	if let (Some(values), Some(properties)) = (value.as_object(), schema.get("properties").and_then(Value::as_object)) {
		let mut transformed = values.clone();
		let existing = existing.and_then(Value::as_object);
		for (key, property_schema) in properties {
			if let Some(property_value) = values.get(key) {
				let previous = existing.and_then(|object| object.get(key));
				transformed.insert(key.clone(), transform_schema_secrets(property_value, property_schema, previous, transform)?);
			}
		}
		return Ok(Value::Object(transformed));
	}

	if let (Some(values), Some(item_schema)) = (value.as_array(), schema.get("items")) {
		let existing = existing.and_then(Value::as_array);
		let transformed = values
			.iter()
			.enumerate()
			.map(|(index, item)| transform_schema_secrets(item, item_schema, existing.and_then(|items| items.get(index)), transform))
			.collect::<Result<Vec<Value>, EncryptionError>>()?;
		return Ok(Value::Array(transformed));
	}

	Ok(value.clone())
}

fn decrypt_current(value: &str, state: &CipherState) -> Result<String, EncryptionError> {
	let (stored_key_id, encoded) = value.split_once(':').ok_or(EncryptionError::InvalidFormat)?;
	if stored_key_id != state.key_id {
		return Err(EncryptionError::KeyMismatch);
	}
	let envelope = BASE64.decode(encoded).map_err(|_| EncryptionError::InvalidFormat)?;
	if envelope.len() <= NONCE_SIZE {
		return Err(EncryptionError::InvalidFormat);
	}
	let (nonce, ciphertext) = envelope.split_at(NONCE_SIZE);
	let plaintext = state
		.cipher
		.decrypt(
			Nonce::from_slice(nonce),
			Payload {
				msg: ciphertext,
				aad: ASSOCIATED_DATA,
			},
		)
		.map_err(|_| EncryptionError::DecryptionFailed)?;
	String::from_utf8(plaintext).map_err(|_| EncryptionError::InvalidPlaintext)
}

fn decrypt_legacy(value: &str, state: &CipherState) -> Result<String, EncryptionError> {
	let envelope = hex_decode(value).map_err(|_| EncryptionError::InvalidFormat)?;
	if envelope.len() <= NONCE_SIZE {
		return Err(EncryptionError::InvalidFormat);
	}
	let split_at = envelope.len() - NONCE_SIZE;
	let (ciphertext, nonce) = envelope.split_at(split_at);
	let plaintext = state
		.cipher
		.decrypt(Nonce::from_slice(nonce), ciphertext)
		.map_err(|_| EncryptionError::DecryptionFailed)?;
	String::from_utf8(plaintext).map_err(|_| EncryptionError::InvalidPlaintext)
}

fn load_or_create_local_key() -> Result<(EncryptionKey, KeySource), EncryptionError> {
	let data_dir = std::env::var_os("OXIDECHAT_DATA_DIR").map_or_else(|| PathBuf::from(DEFAULT_DATA_DIR), PathBuf::from);
	fs::create_dir_all(&data_dir)?;
	let key_path = data_dir.join(KEY_FILE_NAME);
	match read_key_file(&key_path) {
		Ok(key) => Ok((key, KeySource::File)),
		Err(EncryptionError::KeyFile(error)) if error.kind() == ErrorKind::NotFound => create_key_file(&key_path),
		Err(error) => Err(error),
	}
}

fn read_key_file(path: &Path) -> Result<EncryptionKey, EncryptionError> {
	let value = fs::read_to_string(path)?;
	decode_key(value.trim()).map_err(|_| EncryptionError::InvalidKeyFile)
}

fn create_key_file(path: &Path) -> Result<(EncryptionKey, KeySource), EncryptionError> {
	let mut key = [0u8; 32];
	OsRng.fill_bytes(&mut key);
	let encoded = hex_encode(&key);
	let mut options = OpenOptions::new();
	options.write(true).create_new(true);
	#[cfg(unix)]
	{
		use std::os::unix::fs::OpenOptionsExt;
		options.mode(0o600);
	}
	match options.open(path) {
		Ok(mut file) => {
			file.write_all(encoded.as_bytes())?;
			file.write_all(b"\n")?;
			file.sync_all()?;
			Ok((key, KeySource::Generated))
		}
		Err(error) if error.kind() == ErrorKind::AlreadyExists => read_key_file(path).map(|key| (key, KeySource::File)),
		Err(error) => Err(EncryptionError::KeyFile(error)),
	}
}

fn key_id(key: &EncryptionKey) -> String {
	let digest = Sha256::digest(key);
	hex_encode(&digest[..8])
}

fn decode_key(value: &str) -> Result<EncryptionKey, ()> {
	let bytes = hex_decode(value)?;
	let key: EncryptionKey = bytes.try_into().map_err(|_| ())?;
	Ok(key)
}

fn hex_encode(data: &[u8]) -> String {
	let mut output = String::with_capacity(data.len() * 2);
	for byte in data {
		use std::fmt::Write;
		let _ = write!(output, "{byte:02x}");
	}
	output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, ()> {
	if !value.len().is_multiple_of(2) {
		return Err(());
	}
	value
		.as_bytes()
		.chunks_exact(2)
		.map(|pair| {
			let high = hex_digit(pair[0])?;
			let low = hex_digit(pair[1])?;
			Ok((high << 4) | low)
		})
		.collect()
}

fn hex_digit(value: u8) -> Result<u8, ()> {
	match value {
		b'0'..=b'9' => Ok(value - b'0'),
		b'a'..=b'f' => Ok(value - b'a' + 10),
		b'A'..=b'F' => Ok(value - b'A' + 10),
		_ => Err(()),
	}
}

fn random_nonce() -> [u8; NONCE_SIZE] {
	let mut nonce = [0u8; NONCE_SIZE];
	OsRng.fill_bytes(&mut nonce);
	nonce
}
