//! Authentication tests.
//!
//! Tests for password hashing and auth types.

#[cfg(test)]
mod tests {
	use crate::types::{AuthResponse, LoginRequest, RegisterRequest, SetupRequest, UserResponse};

	mod types {
		use super::*;

		#[test]
		fn setup_request_deserializes() {
			let json = r#"{"email":"admin@example.com","username":"admin","password":"secret123"}"#;
			let request: SetupRequest = serde_json::from_str(json).unwrap();
			assert_eq!(request.email, "admin@example.com");
			assert_eq!(request.username, "admin");
			assert_eq!(request.password, "secret123");
		}

		#[test]
		fn register_request_deserializes() {
			let json = r#"{"email":"user@example.com","username":"user","password":"pass123"}"#;
			let request: RegisterRequest = serde_json::from_str(json).unwrap();
			assert_eq!(request.email, "user@example.com");
			assert_eq!(request.username, "user");
		}

		#[test]
		fn login_request_deserializes() {
			let json = r#"{"email":"user@example.com","password":"pass123"}"#;
			let request: LoginRequest = serde_json::from_str(json).unwrap();
			assert_eq!(request.email, "user@example.com");
			assert_eq!(request.password, "pass123");
		}

		#[test]
		fn user_response_serializes() {
			let user = UserResponse {
				id: uuid::Uuid::nil(),
				email: "test@example.com".to_string(),
				username: "testuser".to_string(),
				auth_method: "local".to_string(),
				roles: vec!["admin".to_string()],
				teams: vec![],
				permissions: vec!["settings.profile.view".to_string()],
				preferences: Default::default(),
				created_at: chrono::Utc::now(),
			};

			let json = serde_json::to_string(&user).unwrap();
			assert!(json.contains("\"email\":\"test@example.com\""));
			assert!(json.contains("\"roles\":[\"admin\"]"));
		}

		#[test]
		fn auth_response_serializes() {
			let response = AuthResponse {
				user: UserResponse {
					id: uuid::Uuid::nil(),
					email: "test@example.com".to_string(),
					username: "testuser".to_string(),
					auth_method: "local".to_string(),
					roles: vec!["user".to_string()],
					teams: vec![],
					permissions: vec![],
					preferences: Default::default(),
					created_at: chrono::Utc::now(),
				},
			};

			let json = serde_json::to_string(&response).unwrap();
			assert!(json.contains("\"user\":{"));
		}
	}

	mod password {
		use argon2::{
			Argon2,
			password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
		};

		#[test]
		fn password_hashing_works() {
			let password = "test_password_123";
			let salt = SaltString::generate(&mut OsRng);
			let argon2 = Argon2::default();

			let hash = argon2.hash_password(password.as_bytes(), &salt).unwrap();
			let hash_str = hash.to_string();

			// Verify the hash
			let parsed_hash = PasswordHash::new(&hash_str).unwrap();
			assert!(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok());
		}

		#[test]
		fn wrong_password_fails_verification() {
			let password = "correct_password";
			let wrong_password = "wrong_password";
			let salt = SaltString::generate(&mut OsRng);
			let argon2 = Argon2::default();

			let hash = argon2.hash_password(password.as_bytes(), &salt).unwrap();
			let hash_str = hash.to_string();

			let parsed_hash = PasswordHash::new(&hash_str).unwrap();
			assert!(argon2.verify_password(wrong_password.as_bytes(), &parsed_hash).is_err());
		}
	}
}
