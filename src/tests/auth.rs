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

	mod cookie_secure {
		use crate::utils::auth::cookie_secure_for_base_url;

		#[test]
		fn https_base_url_is_secure() {
			assert!(cookie_secure_for_base_url("https://chat.example.com"));
		}

		#[test]
		fn http_base_url_is_not_secure() {
			assert!(!cookie_secure_for_base_url("http://localhost:8080"));
		}

		#[test]
		fn http_scheme_match_is_case_insensitive() {
			assert!(!cookie_secure_for_base_url("HTTP://localhost:8080"));
		}

		#[test]
		fn missing_scheme_defaults_to_secure() {
			assert!(cookie_secure_for_base_url("example.com"));
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

	mod password_utils {
		use crate::utils::auth::{hash_password, verify_password};

		#[test]
		fn hash_and_verify_roundtrip() {
			let hash = hash_password("Sup3r$ecret-pass").unwrap();
			assert!(verify_password("Sup3r$ecret-pass", &hash).unwrap());
		}

		#[test]
		fn wrong_password_fails_verification() {
			let hash = hash_password("Sup3r$ecret-pass").unwrap();
			assert!(!verify_password("wrong-password", &hash).unwrap());
		}

		#[test]
		fn hash_pins_explicit_argon2id_params() {
			let hash = hash_password("Sup3r$ecret-pass").unwrap();
			assert!(hash.starts_with("$argon2id$v=19$m=65536,t=3,p=1$"), "unexpected hash prefix: {hash}");
		}
	}

	mod rate_limit {
		use crate::utils::rate_limit::RateLimiter;
		use std::time::Duration;

		#[tokio::test]
		async fn allows_up_to_limit_then_blocks() {
			let limiter = RateLimiter::spawn();
			for _ in 0..3 {
				assert!(limiter.allow("key", 3, Duration::from_secs(60)).await);
			}
			assert!(!limiter.allow("key", 3, Duration::from_secs(60)).await);
			assert!(!limiter.allow("key", 3, Duration::from_secs(60)).await);
		}

		#[tokio::test(start_paused = true)]
		async fn resets_after_window() {
			let limiter = RateLimiter::spawn();
			assert!(limiter.allow("key", 1, Duration::from_secs(60)).await);
			assert!(!limiter.allow("key", 1, Duration::from_secs(60)).await);
			tokio::time::advance(Duration::from_secs(61)).await;
			assert!(limiter.allow("key", 1, Duration::from_secs(60)).await);
		}

		#[tokio::test]
		async fn keys_are_independent() {
			let limiter = RateLimiter::spawn();
			assert!(limiter.allow("a", 1, Duration::from_secs(60)).await);
			assert!(!limiter.allow("a", 1, Duration::from_secs(60)).await);
			assert!(limiter.allow("b", 1, Duration::from_secs(60)).await);
		}

		#[tokio::test]
		async fn zero_limit_disables_limiting() {
			let limiter = RateLimiter::spawn();
			for _ in 0..10 {
				assert!(limiter.allow("key", 0, Duration::from_secs(60)).await);
			}
		}
	}

	mod rate_limit_http {
		use axum::body::{Body, to_bytes};
		use axum::http::Request;
		use sqlx::PgPool;
		use tower::ServiceExt;

		#[sqlx::test(migrations = "./migrations")]
		async fn auth_route_returns_429_after_the_per_ip_limit(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
			crate::config::Config::init(&pool).await;
			crate::i18n::I18n::init(&pool).await;
			let state = crate::tests::gateway::support::job_state(pool);
			let router = crate::routes::build_router(state);
			let invalid_setup_body = "{}";
			let mut statuses = Vec::with_capacity(21);
			for _ in 0..21 {
				let request = Request::builder()
					.method("POST")
					.uri("/api/v1/auth/setup")
					.header("content-type", "application/json")
					.body(Body::from(invalid_setup_body))?;
				let response = router.clone().oneshot(request).await?;
				statuses.push(response.status().as_u16());
			}
			assert!(
				statuses[..20].iter().all(|status| *status == 422),
				"expected 422 for the first 20 invalid setup requests: {statuses:?}"
			);
			assert_eq!(statuses[20], 429);
			let request = Request::builder()
				.method("POST")
				.uri("/api/v1/auth/setup")
				.header("content-type", "application/json")
				.body(Body::from(invalid_setup_body))?;
			let response = router.oneshot(request).await?;
			assert_eq!(response.status().as_u16(), 429);
			let payload = to_bytes(response.into_body(), 4096).await?;
			let text = String::from_utf8_lossy(&payload);
			assert!(text.contains("too_many_requests"), "unexpected body: {text}");
			assert!(text.contains("Too many requests"), "missing i18n message: {text}");
			Ok(())
		}
	}

	mod login_limit {
		use crate::utils::login_limit::LoginRateLimiter;
		use std::net::{IpAddr, Ipv4Addr};
		use std::time::Duration;

		const IP_A: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
		const IP_B: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 2));

		#[tokio::test(start_paused = true)]
		async fn failures_up_to_limit_are_allowed_then_rejected() {
			let limiter = LoginRateLimiter::spawn();
			for _ in 0..10 {
				assert!(limiter.allow(IP_A, "a@example.com").await);
				limiter.record_failure(IP_A, "a@example.com");
			}
			assert!(!limiter.allow(IP_A, "a@example.com").await);
			assert!(!limiter.allow(IP_A, "other@example.com").await);
		}

		#[tokio::test(start_paused = true)]
		async fn ip_limit_blocks_all_emails_from_that_ip() {
			let limiter = LoginRateLimiter::spawn();
			for _ in 0..10 {
				limiter.record_failure(IP_A, "a@example.com");
			}
			assert!(!limiter.allow(IP_A, "b@example.com").await);
			assert!(limiter.allow(IP_B, "b@example.com").await);
		}

		#[tokio::test(start_paused = true)]
		async fn email_limit_is_scoped_to_its_own_ip() {
			let limiter = LoginRateLimiter::spawn();
			for _ in 0..10 {
				limiter.record_failure(IP_A, "a@example.com");
			}
			assert!(!limiter.allow(IP_A, "a@example.com").await);
			assert!(limiter.allow(IP_B, "a@example.com").await);
		}

		#[tokio::test(start_paused = true)]
		async fn failures_expire_after_the_window() {
			let limiter = LoginRateLimiter::spawn();
			for _ in 0..10 {
				limiter.record_failure(IP_A, "a@example.com");
			}
			assert!(!limiter.allow(IP_A, "a@example.com").await);
			tokio::time::advance(Duration::from_secs(5 * 60)).await;
			assert!(limiter.allow(IP_A, "a@example.com").await);
		}

		#[tokio::test(start_paused = true)]
		async fn allowed_attempts_do_not_consume_failures() {
			let limiter = LoginRateLimiter::spawn();
			for _ in 0..20 {
				assert!(limiter.allow(IP_A, "a@example.com").await);
			}
			limiter.record_failure(IP_A, "a@example.com");
			assert!(limiter.allow(IP_A, "a@example.com").await);
		}
	}
}
