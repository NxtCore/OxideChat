#[cfg(test)]
mod validation {
	use crate::types::rag::{
		ConnectionKind, ConnectionMode, EmbeddingSettings, RagDefaults, RagError, RerankerSettings, SaveConnectionRequest, UpdateRagSetupRequest, parse_version,
	};
	use uuid::Uuid;

	pub fn request(kind: ConnectionKind, mode: ConnectionMode, url: Option<&str>, secret: Option<&str>) -> SaveConnectionRequest {
		SaveConnectionRequest {
			name: format!("{kind:?} {mode:?}"),
			kind,
			mode,
			url: url.map(str::to_string),
			secret: secret.map(str::to_string),
			embedding: None,
		}
	}

	fn check(request: &SaveConnectionRequest) -> Result<(), &'static str> {
		request.validate(request.kind, request.mode, false).map_err(|error| error.reason())
	}

	#[test]
	fn addresses_match_the_store_and_never_carry_a_password() {
		use ConnectionKind::{Pgvector, Qdrant};
		use ConnectionMode::{Managed, ReadOnly};
		assert_eq!(check(&request(Pgvector, Managed, None, None)), Ok(()));
		assert_eq!(check(&request(Pgvector, ReadOnly, Some("postgres://reader@db.example.com/kb"), Some("pw"))), Ok(()));
		assert_eq!(
			check(&request(Pgvector, ReadOnly, Some("postgres://reader:pw@db.example.com/kb"), None)),
			Err("url_has_password")
		);
		assert_eq!(check(&request(Pgvector, ReadOnly, None, None)), Err("url_required"));
		assert_eq!(check(&request(Pgvector, ReadOnly, Some("  "), None)), Err("url_required"));
		assert_eq!(check(&request(Pgvector, ReadOnly, Some("https://db.example.com"), None)), Err("invalid_url"));
		assert_eq!(check(&request(Qdrant, ReadOnly, Some("https://qdrant.example.com:6333"), Some("key"))), Ok(()));
		assert_eq!(check(&request(Qdrant, ReadOnly, Some("http://qdrant:6333"), None)), Ok(()));
		assert_eq!(check(&request(Qdrant, ReadOnly, Some("http://qdrant:6333"), Some("key"))), Err("insecure_secret"));
		assert_eq!(check(&request(Qdrant, ReadOnly, Some("postgres://qdrant"), None)), Err("invalid_url"));
		assert_eq!(
			check(&request(Qdrant, Managed, Some("https://qdrant.example.com"), None)),
			Err("managed_needs_pgvector")
		);
	}

	#[test]
	fn a_kept_key_also_counts_for_plain_http() {
		let request = request(ConnectionKind::Qdrant, ConnectionMode::ReadOnly, Some("http://qdrant:6333"), None);
		assert!(matches!(request.validate(request.kind, request.mode, true), Err(RagError::InsecureSecret)));
	}

	#[test]
	fn names_and_embedding_settings_are_bounded() {
		let mut managed = request(ConnectionKind::Pgvector, ConnectionMode::Managed, None, None);
		managed.name = " ".to_string();
		assert_eq!(check(&managed), Err("invalid_name"));
		managed.name = "Main".to_string();
		for (dimensions, expected) in [
			(None, Ok(())),
			(Some(1024), Ok(())),
			(Some(0), Err("invalid_dimensions")),
			(Some(4001), Err("invalid_dimensions")),
		] {
			managed.embedding = Some(EmbeddingSettings {
				model_id: Uuid::new_v4(),
				dimensions,
			});
			assert_eq!(check(&managed), expected, "{dimensions:?}");
		}
		let mut read_only = request(ConnectionKind::Qdrant, ConnectionMode::ReadOnly, Some("https://qdrant.example.com"), None);
		read_only.embedding = Some(EmbeddingSettings {
			model_id: Uuid::new_v4(),
			dimensions: None,
		});
		assert_eq!(check(&read_only), Err("read_only_has_model"));
	}

	#[test]
	fn top_n_is_bounded() {
		let setup = |top_n| UpdateRagSetupRequest {
			enabled: true,
			reranker: RerankerSettings { model_id: None, top_n },
			defaults: RagDefaults { connection_id: None },
		};
		assert!(setup(1).validate().is_ok());
		assert!(setup(50).validate().is_ok());
		assert!(setup(0).validate().is_err());
		assert!(setup(51).validate().is_err());
	}

	#[test]
	fn versions_compare_by_major_and_minor() {
		assert_eq!(parse_version("0.8.0"), (0, 8));
		assert_eq!(parse_version("0.10.1"), (0, 10));
		assert!(parse_version("0.6.0") < (0, 8));
		assert!(parse_version("1.0") > (0, 8));
		assert_eq!(parse_version("garbage"), (0, 0));
	}
}

#[cfg(test)]
mod connections {
	use super::validation::request;
	use crate::tests::support::{TestResult, create_user};
	use crate::types::rag::{ConnectionKind, ConnectionMode, RagConfig, RagConnection, RagDefaults, RagError, RerankerSettings, UpdateRagSetupRequest};
	use crate::utils::encryption::decrypt_api_key;
	use sqlx::PgPool;
	use uuid::Uuid;

	async fn read_only(pool: &PgPool, actor: &Uuid) -> TestResult<RagConnection> {
		crate::utils::encryption::init_for_test([0x42; 32])?;
		let request = request(
			ConnectionKind::Pgvector,
			ConnectionMode::ReadOnly,
			Some("postgres://reader@db.example.com/kb"),
			Some("pw"),
		);
		Ok(RagConnection::create(pool, actor, &request, None).await?)
	}

	async fn audit_events(pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
		sqlx::query_scalar("SELECT event FROM audit_logs WHERE target_type IN ('rag_connection', 'rag_config') ORDER BY created_at, event")
			.fetch_all(pool)
			.await
	}

	#[sqlx::test]
	async fn the_secret_is_stored_encrypted_and_never_returned(pool: PgPool) -> TestResult {
		let actor = create_user(&pool).await?;
		let connection = read_only(&pool, &actor).await?;
		let stored = connection.secret.as_deref().ok_or("no secret")?;
		assert_ne!(stored, "pw");
		assert_eq!(decrypt_api_key(stored)?, "pw");
		let response = serde_json::to_value(connection.to_response(false))?;
		assert_eq!(response["has_secret"], true);
		assert!(!response.to_string().contains(stored));
		assert_eq!(audit_events(&pool).await?, ["rag_connection_created"]);
		Ok(())
	}

	#[sqlx::test]
	async fn updates_keep_or_remove_the_secret(pool: PgPool) -> TestResult {
		let actor = create_user(&pool).await?;
		let created = read_only(&pool, &actor).await?;
		let mut change = request(ConnectionKind::Qdrant, ConnectionMode::Managed, Some("postgres://reader@db.example.com/other"), None);
		change.name = "Renamed".to_string();
		let kept = created.update(&pool, &actor, &change, None).await?;
		assert_eq!(
			(kept.name.as_str(), kept.kind, kept.mode),
			("Renamed", ConnectionKind::Pgvector, ConnectionMode::ReadOnly)
		);
		assert_eq!(kept.secret, created.secret);
		change.secret = Some(String::new());
		assert_eq!(kept.update(&pool, &actor, &change, None).await?.secret, None);
		Ok(())
	}

	#[sqlx::test]
	async fn a_new_host_needs_the_secret_again(pool: PgPool) -> TestResult {
		let actor = create_user(&pool).await?;
		let created = read_only(&pool, &actor).await?;
		let moved = |url: &str, secret: Option<&str>| request(ConnectionKind::Pgvector, ConnectionMode::ReadOnly, Some(url), secret);
		assert!(!created.redirects_secret(&moved("postgres://reader@db.example.com/other_db", None)));
		assert!(created.redirects_secret(&moved("postgres://reader@evil.example/kb", None)));
		assert!(created.redirects_secret(&moved("postgres://reader@db.example.com:5433/kb", None)));
		assert!(!created.redirects_secret(&moved("postgres://reader@evil.example/kb", Some("retyped"))));
		Ok(())
	}

	#[sqlx::test]
	async fn a_new_vector_size_starts_a_new_index_generation(pool: PgPool) -> TestResult {
		let actor = create_user(&pool).await?;
		let managed = RagConnection::create(&pool, &actor, &request(ConnectionKind::Pgvector, ConnectionMode::Managed, None, None), None).await?;
		let same = managed
			.update(&pool, &actor, &request(ConnectionKind::Pgvector, ConnectionMode::Managed, None, None), None)
			.await?;
		assert_eq!(same.index_generation, 0);
		let resized = same
			.update(&pool, &actor, &request(ConnectionKind::Pgvector, ConnectionMode::Managed, None, None), Some(768))
			.await?;
		assert_eq!((resized.index_generation, resized.vector_size), (1, Some(768)));
		Ok(())
	}

	#[sqlx::test]
	async fn the_default_storage_must_be_managed(pool: PgPool) -> TestResult {
		let actor = create_user(&pool).await?;
		let external = read_only(&pool, &actor).await?;
		let managed = RagConnection::create(&pool, &actor, &request(ConnectionKind::Pgvector, ConnectionMode::Managed, None, None), None).await?;
		let setup = |connection_id| UpdateRagSetupRequest {
			enabled: true,
			reranker: RerankerSettings { model_id: None, top_n: 6 },
			defaults: RagDefaults { connection_id },
		};

		assert!(matches!(
			RagConfig::save(&pool, &actor, &setup(Some(external.id))).await,
			Err(RagError::DefaultNotManaged)
		));
		assert!(matches!(RagConfig::save(&pool, &actor, &setup(Some(Uuid::new_v4()))).await, Err(RagError::NotFound)));
		RagConfig::save(&pool, &actor, &setup(Some(managed.id))).await?;
		let config = RagConfig::get(&pool).await?;
		assert!(config.enabled);
		assert_eq!(config.default_connection_id, Some(managed.id));

		assert!(RagConnection::delete(&pool, &actor, &managed.id).await?);
		assert_eq!(RagConfig::get(&pool).await?.default_connection_id, None);
		assert!(!RagConnection::delete(&pool, &actor, &managed.id).await?);
		assert!(audit_events(&pool).await?.contains(&"rag_config_updated".to_string()));
		Ok(())
	}

	#[sqlx::test]
	async fn names_are_unique(pool: PgPool) -> TestResult {
		let actor = create_user(&pool).await?;
		read_only(&pool, &actor).await?;
		let duplicate = read_only(&pool, &actor).await;
		assert!(duplicate.is_err());
		Ok(())
	}
}

#[cfg(test)]
mod stores {
	use super::validation::request;
	use crate::tests::support::{TestResult, create_user};
	use crate::types::rag::{ConnectionKind, ConnectionMode, RagConnection, StoreError, VectorStore};
	use sqlx::PgPool;

	async fn connection(pool: &PgPool, kind: ConnectionKind, mode: ConnectionMode, url: Option<&str>) -> TestResult<RagConnection> {
		let actor = create_user(pool).await?;
		Ok(RagConnection::create(pool, &actor, &request(kind, mode, url, None), None).await?)
	}

	#[sqlx::test]
	async fn the_main_database_needs_pgvector(pool: PgPool) -> TestResult {
		sqlx::query("DROP EXTENSION IF EXISTS vector").execute(&pool).await?;
		let managed = connection(&pool, ConnectionKind::Pgvector, ConnectionMode::Managed, None).await?;
		let store = VectorStore::connect(&managed, &pool, None).await?;
		assert_eq!(store.health().await.err(), Some(StoreError::ExtensionMissing));
		Ok(())
	}

	#[sqlx::test]
	async fn private_addresses_are_refused(pool: PgPool) -> TestResult {
		if crate::utils::outbound::private_hosts_allowed() {
			return Ok(());
		}
		let postgres = connection(&pool, ConnectionKind::Pgvector, ConnectionMode::ReadOnly, Some("postgres://reader@127.0.0.1:5432/kb")).await?;
		assert!(matches!(VectorStore::connect(&postgres, &pool, None).await, Err(StoreError::Blocked)));
		let qdrant = connection(&pool, ConnectionKind::Qdrant, ConnectionMode::ReadOnly, Some("http://127.0.0.1:6333")).await?;
		assert!(matches!(VectorStore::connect(&qdrant, &pool, None).await, Err(StoreError::InvalidUrl)));
		Ok(())
	}
}
