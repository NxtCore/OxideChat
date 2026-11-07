//! OAuth tests.
//!
//! Tests for claim mapping, the outbound address guard, provider requests and the sign-in rules.

#[cfg(test)]
mod tests {
	mod claims {
		use crate::types::oauth::{ClaimMapping, OAuthUserInfo};
		use serde_json::json;

		#[test]
		fn default_mapping_reads_oidc_claims() {
			let profile = json!({"sub":"123","email":"user@example.com","email_verified":true,"name":"User"});
			let info = OAuthUserInfo::from_profile("google", &ClaimMapping::default(), &profile).unwrap();

			assert_eq!(info.provider, "google");
			assert_eq!(info.provider_user_id, "123");
			assert_eq!(info.email.as_deref(), Some("user@example.com"));
			assert_eq!(info.email_verified, Some(true));
			assert_eq!(info.username.as_deref(), Some("User"));
		}

		#[test]
		fn custom_mapping_reads_other_names_and_nested_claims() {
			let mapping = ClaimMapping {
				subject: "id".to_string(),
				email_verified: "verified".to_string(),
				name: "profile.display".to_string(),
				..ClaimMapping::default()
			};
			let profile = json!({"id":42,"email":"user@example.com","verified":"false","profile":{"display":"User"}});
			let info = OAuthUserInfo::from_profile("discord", &mapping, &profile).unwrap();

			assert_eq!(info.provider_user_id, "42");
			assert_eq!(info.email_verified, Some(false));
			assert_eq!(info.username.as_deref(), Some("User"));
		}

		#[test]
		fn missing_values_stay_none() {
			let info = OAuthUserInfo::from_profile("p", &ClaimMapping::default(), &json!({"sub":"1","email":null})).unwrap();

			assert_eq!(info.email, None);
			assert_eq!(info.email_verified, None);
			assert_eq!(info.username, None);
		}

		#[test]
		fn malformed_emails_are_ignored() {
			let info = OAuthUserInfo::from_profile("p", &ClaimMapping::default(), &json!({"sub":"1","email":"not-an-email"})).unwrap();

			assert_eq!(info.email, None);
		}

		#[test]
		fn a_profile_without_subject_is_refused() {
			assert!(OAuthUserInfo::from_profile("p", &ClaimMapping::default(), &json!({"email":"user@example.com"})).is_none());
			assert!(OAuthUserInfo::from_profile("p", &ClaimMapping::default(), &json!({"sub":"x".repeat(256)})).is_none());
			assert!(OAuthUserInfo::from_profile("p", &ClaimMapping::default(), &json!({"sub":"  "})).is_none());
		}

		#[test]
		fn mapping_fills_omitted_fields_with_defaults() {
			let mapping: ClaimMapping = serde_json::from_str(r#"{"email":"mail"}"#).unwrap();

			assert_eq!(mapping.email, "mail");
			assert_eq!(mapping.subject, "sub");
		}
	}

	mod outbound {
		use crate::utils::outbound::is_allowed_ip;
		use std::net::IpAddr;

		fn allowed(ip: &str, allow_private: bool) -> bool {
			is_allowed_ip(ip.parse::<IpAddr>().unwrap(), allow_private)
		}

		#[test]
		fn public_addresses_are_allowed() {
			assert!(allowed("8.8.8.8", false));
			assert!(allowed("2606:4700:4700::1111", false));
			assert!(allowed("64:ff9b::808:808", false));
			assert!(allowed("2002:808:808::", false));
		}

		#[test]
		fn metadata_and_link_local_addresses_are_never_allowed() {
			for ip in [
				"169.254.169.254",
				"fe80::1",
				"::ffff:169.254.169.254",
				"0.0.0.0",
				"0.1.2.3",
				"224.0.0.1",
				"240.0.0.1",
				"64:ff9b::a9fe:a9fe",
				"2002:a9fe:a9fe::",
				"::169.254.169.254",
				"2001:0:4136:e378::1",
			] {
				assert!(!allowed(ip, false), "{ip}");
				assert!(!allowed(ip, true), "{ip}");
			}
		}

		#[test]
		fn private_addresses_need_the_opt_in() {
			for ip in [
				"127.0.0.1",
				"10.0.0.5",
				"172.16.0.1",
				"192.168.1.1",
				"100.64.0.1",
				"192.0.0.8",
				"198.18.0.1",
				"::1",
				"fd00::1",
				"::ffff:10.0.0.1",
				"::127.0.0.1",
				"2002:7f00:1::",
				"64:ff9b::a00:5",
			] {
				assert!(!allowed(ip, false), "{ip}");
				assert!(allowed(ip, true), "{ip}");
			}
		}
	}

	mod provider_requests {
		use crate::types::oauth::{OAuthProvider, SaveOAuthProviderRequest, is_valid_slug};

		fn request(json: &str) -> SaveOAuthProviderRequest {
			serde_json::from_str(json).unwrap()
		}

		const VALID: &str = r#"{"slug":"keycloak","name":"Keycloak","endpoints":{"issuer_url":"https://id.example.com"},"client_id":"app","client_secret":"s3cret","scopes":["openid"]}"#;

		#[test]
		fn slugs_are_short_lowercase_and_url_safe() {
			assert!(is_valid_slug("keycloak"));
			assert!(is_valid_slug("entra-2"));
			for slug in ["", "-a", "Google", "a_b", "a b", "a/b", &"a".repeat(51)] {
				assert!(!is_valid_slug(slug), "{slug}");
			}
		}

		#[test]
		fn defaults_allow_sign_up_but_do_not_trust_emails() {
			let req = request(VALID);

			assert!(req.allow_registration);
			assert!(req.enabled);
			assert!(!req.trust_email);
			assert!(req.is_valid(true));
		}

		#[test]
		fn creating_needs_a_secret_and_a_valid_slug_but_updating_does_not() {
			let without_secret = request(r#"{"slug":"keycloak","name":"K","endpoints":{},"client_id":"app","scopes":[]}"#);
			let bad_slug = request(&VALID.replace("keycloak", "Key Cloak"));

			assert!(!without_secret.is_valid(true));
			assert!(without_secret.is_valid(false));
			assert!(!bad_slug.is_valid(true));
			assert!(bad_slug.is_valid(false));
		}

		#[test]
		fn icons_longer_than_the_column_are_refused() {
			let mut req = request(VALID);
			req.icon = Some("i".repeat(51));

			assert!(!req.is_valid(true));
		}

		#[test]
		fn names_and_client_ids_must_not_be_empty() {
			assert!(!request(&VALID.replace(r#""name":"Keycloak""#, r#""name":"  ""#)).is_valid(false));
			assert!(!request(&VALID.replace(r#""client_id":"app""#, r#""client_id":"""#)).is_valid(false));
		}

		#[test]
		fn presets_cover_google_and_discord_with_endpoints() {
			let presets = OAuthProvider::presets();
			let slugs: Vec<&str> = presets.iter().map(|preset| preset.slug.as_str()).collect();

			assert_eq!(slugs, ["google", "discord"]);
			assert!(presets.iter().all(|preset| preset.endpoints.token_url.is_some() && !preset.scopes.is_empty()));
			assert_eq!(presets[1].claim_mapping.subject, "id");
		}
	}

	mod providers {
		use crate::tests::support::TestResult;
		use crate::types::oauth::{OAuthEndpoints, OAuthProvider, OAuthUserInfo, ResolvedEndpoints, SaveOAuthProviderRequest, SignInError, SignInKind};
		use sqlx::PgPool;

		fn request(slug: &str) -> SaveOAuthProviderRequest {
			SaveOAuthProviderRequest {
				slug: slug.to_string(),
				name: slug.to_string(),
				icon: None,
				endpoints: OAuthEndpoints::default(),
				client_id: "app".to_string(),
				client_secret: Some("s3cret".to_string()),
				scopes: vec!["openid".to_string()],
				claim_mapping: Default::default(),
				trust_email: false,
				allow_registration: true,
				enabled: true,
			}
		}

		fn endpoints() -> ResolvedEndpoints {
			ResolvedEndpoints {
				issuer_url: None,
				authorization_url: "https://id.example.com/auth".to_string(),
				token_url: "https://id.example.com/token".to_string(),
				userinfo_url: "https://id.example.com/userinfo".to_string(),
			}
		}

		async fn provider(pool: &PgPool, req: SaveOAuthProviderRequest) -> TestResult<OAuthProvider> {
			crate::utils::encryption::init_for_test([0x42; 32])?;
			Ok(OAuthProvider::create(pool, &req, &endpoints()).await?)
		}

		fn info(provider: &OAuthProvider, id: &str, email: &str, verified: Option<bool>) -> OAuthUserInfo {
			OAuthUserInfo {
				provider: provider.slug.clone(),
				provider_user_id: id.to_string(),
				email: Some(email.to_string()),
				email_verified: verified,
				username: Some("Sam".to_string()),
				raw_data: serde_json::json!({ "sub": id }),
			}
		}

		async fn sign_in(pool: &PgPool, provider: &OAuthProvider, info: &OAuthUserInfo) -> TestResult<(crate::types::User, SignInKind)> {
			provider.sign_in(pool, info).await.map_err(|error| format!("{error:?}").into())
		}

		#[sqlx::test]
		async fn the_secret_is_stored_encrypted_and_never_shown(pool: PgPool) -> TestResult {
			let provider = provider(&pool, request("keycloak")).await?;

			assert_ne!(provider.client_secret, "s3cret");
			assert_eq!(provider.plain_client_secret()?, "s3cret");
			assert!(serde_json::to_value(provider.to_response())?.get("client_secret").is_none());
			Ok(())
		}

		#[sqlx::test]
		async fn an_update_without_a_secret_keeps_the_stored_one(pool: PgPool) -> TestResult {
			let created = provider(&pool, request("keycloak")).await?;
			let mut change = request("keycloak");
			change.name = "Renamed".to_string();
			change.client_secret = Some(String::new());
			let updated = OAuthProvider::update(&pool, created.id, &change, &endpoints()).await?.ok_or("missing")?;

			assert_eq!(updated.name, "Renamed");
			assert_eq!(updated.client_secret, created.client_secret);
			Ok(())
		}

		#[sqlx::test]
		async fn only_enabled_providers_are_found_by_their_login_key(pool: PgPool) -> TestResult {
			let mut off = request("off");
			off.enabled = false;
			provider(&pool, request("on")).await?;
			provider(&pool, off).await?;

			assert!(OAuthProvider::find_enabled_by_slug(&pool, "on").await?.is_some());
			assert!(OAuthProvider::find_enabled_by_slug(&pool, "off").await?.is_none());
			assert_eq!(OAuthProvider::list_enabled(&pool).await?.len(), 1);
			Ok(())
		}

		#[sqlx::test]
		async fn a_slug_can_only_be_used_once(pool: PgPool) -> TestResult {
			provider(&pool, request("keycloak")).await?;
			let again = OAuthProvider::create(&pool, &request("keycloak"), &endpoints()).await;

			assert!(again.is_err_and(|error| crate::types::error::is_unique_violation(&error)));
			Ok(())
		}

		#[sqlx::test]
		async fn a_verified_login_creates_an_account_and_returns_to_it(pool: PgPool) -> TestResult {
			let provider = provider(&pool, request("keycloak")).await?;
			let (user, kind) = sign_in(&pool, &provider, &info(&provider, "1", "sam@example.com", Some(true))).await?;
			let (again, again_kind) = sign_in(&pool, &provider, &info(&provider, "1", "other@example.com", Some(false))).await?;

			assert_eq!(kind, SignInKind::Created);
			assert_eq!(again_kind, SignInKind::Existing);
			assert_eq!(user.id, again.id);
			Ok(())
		}

		#[sqlx::test]
		async fn a_verified_email_links_to_the_existing_account(pool: PgPool) -> TestResult {
			let owner = crate::tests::support::insert_user(&pool, "sam").await?;
			let provider = provider(&pool, request("keycloak")).await?;
			let (user, kind) = sign_in(&pool, &provider, &info(&provider, "1", "sam@example.com", Some(true))).await?;

			assert_eq!(kind, SignInKind::Linked);
			assert_eq!(user.id, owner);
			Ok(())
		}

		#[sqlx::test]
		async fn an_unverified_email_never_links_or_creates(pool: PgPool) -> TestResult {
			crate::tests::support::insert_user(&pool, "sam").await?;
			let provider = provider(&pool, request("keycloak")).await?;

			for verified in [Some(false), None] {
				let result = provider.sign_in(&pool, &info(&provider, "1", "sam@example.com", verified)).await;
				assert!(matches!(result, Err(SignInError::EmailNotVerified)));
			}
			Ok(())
		}

		#[sqlx::test]
		async fn trusting_a_provider_accepts_a_missing_flag_but_not_an_explicit_false(pool: PgPool) -> TestResult {
			let mut trusted = request("entra");
			trusted.trust_email = true;
			let provider = provider(&pool, trusted).await?;

			assert!(provider.sign_in(&pool, &info(&provider, "1", "a@example.com", None)).await.is_ok());
			let refused = provider.sign_in(&pool, &info(&provider, "2", "b@example.com", Some(false))).await;
			assert!(matches!(refused, Err(SignInError::EmailNotVerified)));
			Ok(())
		}

		#[sqlx::test]
		async fn a_provider_can_forbid_new_accounts_but_still_signs_in_known_ones(pool: PgPool) -> TestResult {
			let mut closed = request("closed");
			closed.allow_registration = false;
			let provider = provider(&pool, closed).await?;
			crate::tests::support::insert_user(&pool, "sam").await?;

			let refused = provider.sign_in(&pool, &info(&provider, "9", "new@example.com", Some(true))).await;
			assert!(matches!(refused, Err(SignInError::RegistrationClosed)));
			assert!(provider.sign_in(&pool, &info(&provider, "1", "sam@example.com", Some(true))).await.is_ok());
			Ok(())
		}

		#[sqlx::test]
		async fn names_that_are_taken_get_a_number(pool: PgPool) -> TestResult {
			crate::tests::support::insert_user(&pool, "Sam").await?;
			let provider = provider(&pool, request("keycloak")).await?;
			let (user, _) = sign_in(&pool, &provider, &info(&provider, "1", "other@example.com", Some(true))).await?;

			assert_eq!(user.username, "Sam1");
			Ok(())
		}

		#[sqlx::test]
		async fn emails_link_regardless_of_case(pool: PgPool) -> TestResult {
			let owner = crate::tests::support::insert_user(&pool, "sam").await?;
			let provider = provider(&pool, request("keycloak")).await?;
			let (user, kind) = sign_in(&pool, &provider, &info(&provider, "1", "Sam@Example.COM", Some(true))).await?;

			assert_eq!(kind, SignInKind::Linked);
			assert_eq!(user.id, owner);
			Ok(())
		}

		#[sqlx::test]
		async fn display_names_become_valid_usernames(pool: PgPool) -> TestResult {
			let provider = provider(&pool, request("keycloak")).await?;
			let mut spaced = info(&provider, "1", "a@example.com", Some(true));
			spaced.username = Some(format!("Sam Smith! {}", "x".repeat(60)));
			let mut symbols = info(&provider, "2", "b@example.com", Some(true));
			symbols.username = Some("!!".to_string());
			let (user, _) = sign_in(&pool, &provider, &spaced).await?;
			let (other, _) = sign_in(&pool, &provider, &symbols).await?;

			assert!(user.username.starts_with("SamSmithx"));
			assert!(crate::utils::auth::validate_username(&user.username).is_ok());
			assert_eq!(other.username, "user_2");
			Ok(())
		}

		#[sqlx::test]
		async fn a_second_account_cannot_differ_only_in_email_case(pool: PgPool) -> TestResult {
			crate::tests::support::insert_user(&pool, "sam").await?;

			assert!(crate::types::User::email_exists(&pool, "SAM@example.com", None).await?);
			let duplicate = crate::types::User::create(&pool, "SAM@example.com", "sam2", "hash").await;
			assert!(duplicate.is_err_and(|error| crate::types::error::is_unique_violation(&error)));
			Ok(())
		}
	}
}
