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
		use ConnectionKind::Pgvector;
		use ConnectionMode::{Managed, ReadOnly};
		assert_eq!(check(&request(Pgvector, Managed, None, None)), Ok(()));
		assert_eq!(check(&request(Pgvector, ReadOnly, Some("postgres://reader@db.example.com/kb"), Some("pw"))), Ok(()));
		assert_eq!(
			check(&request(Pgvector, ReadOnly, Some("postgres://reader:pw@db.example.com/kb"), None)),
			Err("url_has_password")
		);
		assert_eq!(
			check(&request(Pgvector, ReadOnly, Some("postgres://reader@db.example.com/kb?sslmode=require"), Some("pw"))),
			Ok(())
		);
		assert_eq!(
			check(&request(Pgvector, ReadOnly, Some("postgres://reader@db.example.com/kb?password=pw"), None)),
			Err("url_has_password")
		);
		for url in [
			"postgres://reader@db.example.com/kb?host=attacker.example.com",
			"postgres://reader@db.example.com/kb?hostaddr=203.0.113.9",
			"postgres://reader@db.example.com/kb?port=6543",
			"postgres://reader@db.example.com/kb?host=/var/run/postgresql",
			"postgres://reader@db.example.com/kb?sslrootcert=/etc/shadow",
			"postgres://reader@%2Fvar%2Frun%2Fpostgresql/kb",
		] {
			assert_eq!(check(&request(Pgvector, ReadOnly, Some(url), Some("pw"))), Err("invalid_url"), "{url}");
		}
		assert_eq!(check(&request(Pgvector, ReadOnly, None, None)), Err("url_required"));
		assert_eq!(check(&request(Pgvector, ReadOnly, Some("  "), None)), Err("url_required"));
		assert_eq!(check(&request(Pgvector, ReadOnly, Some("https://db.example.com"), None)), Err("invalid_url"));
	}

	#[test]
	fn qdrant_keys_need_https_and_managed_storage_needs_pgvector() {
		use ConnectionKind::Qdrant;
		use ConnectionMode::{Managed, ReadOnly};
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
			reranker: RerankerSettings { model_id: None, top_n },
			defaults: RagDefaults { connection_id: None },
			chunking: None,
			quotas: None,
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
	async fn a_secret_that_looks_encrypted_is_still_encrypted(pool: PgPool) -> TestResult {
		let actor = create_user(&pool).await?;
		crate::utils::encryption::init_for_test([0x42; 32])?;
		let ciphertext = read_only(&pool, &actor).await?.secret.ok_or("no secret")?;
		for secret in [ciphertext.as_str(), "enc:not-a-ciphertext", "***"] {
			let mut create = request(ConnectionKind::Pgvector, ConnectionMode::ReadOnly, Some("postgres://reader@db.example.com/kb"), Some(secret));
			create.name = format!("Copy {}", secret.len());
			let stored = RagConnection::create(&pool, &actor, &create, None).await?.secret.ok_or("no secret")?;
			assert_ne!(stored, secret);
			assert_eq!(decrypt_api_key(&stored)?, secret);
		}
		Ok(())
	}

	#[sqlx::test]
	async fn the_stored_address_is_checked_again_when_saving(pool: PgPool) -> TestResult {
		let actor = create_user(&pool).await?;
		let created = read_only(&pool, &actor).await?;
		let moved = request(ConnectionKind::Pgvector, ConnectionMode::ReadOnly, Some("postgres://reader@evil.example/kb"), None);
		assert!(matches!(created.update(&pool, &actor, &moved, None).await, Err(RagError::SecretRequiredForNewHost)));
		let downgraded = request(ConnectionKind::Pgvector, ConnectionMode::ReadOnly, Some("postgres://reader@db.example.com/kb?sslmode=disable"), None);
		assert!(created.redirects_secret(&downgraded));
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
			reranker: RerankerSettings { model_id: None, top_n: 6 },
			defaults: RagDefaults { connection_id },
			chunking: None,
			quotas: None,
		};

		assert!(matches!(
			RagConfig::save(&pool, &actor, &setup(Some(external.id))).await,
			Err(RagError::DefaultNotManaged)
		));
		assert!(matches!(RagConfig::save(&pool, &actor, &setup(Some(Uuid::new_v4()))).await, Err(RagError::NotFound)));
		RagConfig::save(&pool, &actor, &setup(Some(managed.id))).await?;
		assert_eq!(RagConfig::get(&pool).await?.default_connection_id, Some(managed.id));

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

#[cfg(test)]
mod views {
	use crate::types::rag::{
		Distance, EmbeddingSettings, ExternalView, FieldFilter, FieldMapping, field_paths, field_text, pg_access, pg_filter, qdrant_access, qdrant_filter, shorten,
	};
	use serde_json::json;
	use uuid::Uuid;

	pub fn view(collection: &str, vector_name: Option<&str>) -> ExternalView {
		ExternalView {
			connection_id: Uuid::nil(),
			collection: collection.to_string(),
			vector_name: vector_name.map(str::to_string),
			distance: Distance::Cosine,
			embedding: EmbeddingSettings {
				model_id: Uuid::nil(),
				dimensions: None,
			},
			fields: FieldMapping {
				text: "page_content".to_string(),
				title: Some("metadata.title".to_string()),
				url: Some("metadata.url".to_string()),
				section: None,
			},
			filter: Vec::new(),
			access: None,
		}
	}

	#[test]
	fn fields_are_read_by_dotted_path() {
		let payload = json!({"page_content": "Holidays", "metadata": {"title": "HR", "page": 4, "tags": ["a"]}});
		assert_eq!(field_text(&payload, "page_content").as_deref(), Some("Holidays"));
		assert_eq!(field_text(&payload, "metadata.title").as_deref(), Some("HR"));
		assert_eq!(field_text(&payload, "metadata.page").as_deref(), Some("4"));
		assert_eq!(field_text(&payload, "metadata.tags"), None);
		assert_eq!(field_text(&payload, "missing.path"), None);
		assert_eq!(
			field_paths(&[payload, json!({"other": true})]),
			["metadata.page", "metadata.tags", "metadata.title", "other", "page_content"]
		);
	}

	#[test]
	fn hits_need_text_and_only_keep_web_links() -> Result<(), &'static str> {
		let view = view("docs", None);
		assert!(view.hit(&json!({"page_content": "  "}), 1.0).is_none());
		let hit = view
			.hit(&json!({"page_content": "Text", "metadata": {"title": "T", "url": "javascript:alert(1)"}}), 0.5)
			.ok_or("no hit")?;
		assert_eq!((hit.title.as_deref(), hit.url), (Some("T"), None));
		let linked = view
			.hit(&json!({"page_content": "Text", "metadata": {"url": "https://intranet.example/hr"}}), 0.5)
			.ok_or("no hit")?;
		assert_eq!(linked.url.as_deref(), Some("https://intranet.example/hr"));
		Ok(())
	}

	#[test]
	fn pgvector_filters_only_use_known_columns_and_parameters() {
		let columns = vec!["site".to_string(), "weird\"name".to_string()];
		let filter = |field: &str, value| vec![FieldFilter { field: field.to_string(), value }];
		assert_eq!(pg_filter(&[], &columns, 2), Some((String::new(), Vec::new())));
		assert_eq!(
			pg_filter(&filter("site", json!("HR")), &columns, 2),
			Some((" WHERE to_jsonb(t.\"site\") = $2::jsonb".to_string(), vec!["\"HR\"".to_string()]))
		);
		assert_eq!(
			pg_filter(&filter("weird\"name", json!(true)), &columns, 4),
			Some((" WHERE to_jsonb(t.\"weird\"\"name\") = $4::jsonb".to_string(), vec!["true".to_string()]))
		);
		assert_eq!(
			pg_filter(&filter("site.meta.level", json!(5)), &columns, 2),
			Some((" WHERE to_jsonb(t.\"site\") @> $2::jsonb".to_string(), vec![r#"{"meta":{"level":5}}"#.to_string()]))
		);
		assert_eq!(pg_filter(&filter("site; DROP TABLE x", json!("HR")), &columns, 2), None);
		assert_eq!(pg_filter(&filter("other.site", json!("HR")), &columns, 2), None);
	}

	#[test]
	fn qdrant_filters_match_every_condition() {
		assert_eq!(qdrant_filter(&[], None), None);
		let filter = vec![FieldFilter {
			field: "metadata.site".to_string(),
			value: json!("HR"),
		}];
		assert_eq!(qdrant_filter(&filter, None), Some(json!({"must": [{"key": "metadata.site", "match": {"value": "HR"}}]})));
		let labels = vec!["HR Members".to_string()];
		assert_eq!(
			qdrant_filter(&filter, Some(qdrant_access("oxide_access", &labels))),
			Some(json!({"must": [
				{"key": "metadata.site", "match": {"value": "HR"}},
				{"key": "oxide_access", "match": {"any": ["HR Members"]}}
			]}))
		);
		assert_eq!(qdrant_filter(&[], Some(qdrant_access("acl", &[]))), Some(json!({"must": [{"key": "acl", "match": {"any": []}}]})));
	}

	#[test]
	fn access_filters_match_inside_the_query_and_only_known_columns() {
		let columns = vec![
			("oxide_access".to_string(), "_text".to_string(), -1),
			("metadata".to_string(), "jsonb".to_string(), -1),
			("groups".to_string(), "_varchar".to_string(), -1),
		];
		assert_eq!(pg_access("oxide_access", &columns, 4, 5), Some(("t.\"oxide_access\" && $4::text[]".to_string(), Vec::new())));
		assert_eq!(pg_access("groups", &columns, 4, 5), Some(("t.\"groups\"::text[] && $4::text[]".to_string(), Vec::new())));
		assert_eq!(
			pg_access("metadata.acl.groups", &columns, 4, 5),
			Some(("(jsonb_typeof((to_jsonb(t.\"metadata\") #> $5::text[])) IN ('array', 'string') AND (to_jsonb(t.\"metadata\") #> $5::text[]) ?| $4::text[])".to_string(), vec!["acl".to_string(), "groups".to_string()]))
		);
		assert_eq!(
			pg_access("metadata", &columns, 4, 5),
			Some(("(jsonb_typeof((to_jsonb(t.\"metadata\") #> $5::text[])) IN ('array', 'string') AND (to_jsonb(t.\"metadata\") #> $5::text[]) ?| $4::text[])".to_string(), Vec::new()))
		);
		assert_eq!(pg_access("unknown", &columns, 4, 5), None);
		assert_eq!(pg_access("x; DROP TABLE y", &columns, 4, 5), None);
	}

	#[test]
	fn views_are_checked_before_any_store_is_asked() {
		let reason = |view: &ExternalView| view.validate().map_err(|error| error.reason());
		assert_eq!(reason(&view("docs", None)), Ok(()));
		assert_eq!(reason(&view(" ", None)), Err("invalid_collection"));
		let mut no_text = view("docs", None);
		no_text.fields.text = String::new();
		assert_eq!(reason(&no_text), Err("invalid_field"));
		let mut nested = view("docs", None);
		nested.filter = vec![FieldFilter {
			field: "site".to_string(),
			value: json!({"$ne": "HR"}),
		}];
		assert_eq!(reason(&nested), Err("invalid_filter"));
		let mut access = view("docs", None);
		access.access = Some(" ".to_string());
		assert_eq!(reason(&access), Err("invalid_field"));
	}

	#[test]
	fn sample_text_is_cut() {
		let long = "x".repeat(600);
		let cut = shorten(json!({"text": long, "list": [long]}));
		assert_eq!(cut["text"].as_str().map(|text| text.chars().count()), Some(501));
		assert_eq!(cut["list"][0].as_str().map(|text| text.chars().count()), Some(501));
	}
}

#[cfg(test)]
mod knowledge_bases {
	use super::validation::request;
	use super::views::view;
	use crate::tests::support::{TestResult, create_user, insert_user};
	use crate::types::rag::{ConnectionKind, ConnectionMode, KnowledgeActor, KnowledgeBase, RagConnection, RagError, SaveKnowledgeBaseRequest};
	use crate::types::{AssignmentKind, AssignmentRef, AssignmentRole, ReplaceAssignmentsRequest};
	use sqlx::PgPool;
	use uuid::Uuid;

	struct Fixture {
		admin: KnowledgeActor,
		connection: RagConnection,
		base: Uuid,
	}

	fn user(user_id: Uuid) -> KnowledgeActor {
		KnowledgeActor::basic(user_id, false, false)
	}

	async fn fixture(pool: &PgPool) -> TestResult<Fixture> {
		let admin = KnowledgeActor::basic(create_user(pool).await?, true, true);
		let connection = RagConnection::create(
			pool,
			&admin.user_id,
			&request(ConnectionKind::Qdrant, ConnectionMode::ReadOnly, Some("https://qdrant.example.com"), None),
			None,
		)
		.await?;
		let mut external = view("docs", None);
		external.connection_id = connection.id;
		let save = SaveKnowledgeBaseRequest {
			name: " HR handbook ".to_string(),
			description: Some(" ".to_string()),
			view: Some(external.clone()),
			..Default::default()
		};
		let base = KnowledgeBase::create(pool, &admin, &save, &external).await?;
		Ok(Fixture { admin, connection, base })
	}

	fn targets(targets: &[(AssignmentKind, Option<Uuid>, Option<AssignmentRole>)]) -> ReplaceAssignmentsRequest {
		ReplaceAssignmentsRequest {
			targets: targets.iter().map(|&(kind, id, role)| AssignmentRef { kind, id, role }).collect(),
		}
	}

	async fn visible(pool: &PgPool, actor: &KnowledgeActor) -> Result<Vec<Uuid>, sqlx::Error> {
		Ok(KnowledgeBase::list(pool, actor).await?.into_iter().map(|base| base.id).collect())
	}

	#[sqlx::test]
	async fn a_new_knowledge_base_is_shared_with_nobody(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let stranger = user(insert_user(&pool, "stranger").await?);
		assert!(visible(&pool, &stranger).await?.is_empty());
		assert!(KnowledgeBase::find(&pool, &stranger, &fixture.base).await?.is_none());
		let base = KnowledgeBase::find(&pool, &fixture.admin, &fixture.base).await?.ok_or("missing")?;
		assert_eq!((base.name.as_str(), base.description.as_deref()), ("HR handbook", None));
		let response = base.to_response(&stranger);
		assert!(response.view.is_none() && response.connection.is_none() && !response.can_edit);
		let auditor = KnowledgeActor {
			view_all: true,
			..user(insert_user(&pool, "auditor").await?)
		};
		let seen = KnowledgeBase::find(&pool, &auditor, &fixture.base).await?.ok_or("missing")?;
		assert!(!auditor.can_search(&seen));
		assert!(fixture.admin.can_search(&seen));
		Ok(())
	}

	#[sqlx::test]
	async fn shares_reach_users_through_teams_roles_and_everyone(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let member = user(insert_user(&pool, "member").await?);
		let holder = user(insert_user(&pool, "holder").await?);
		let named = user(insert_user(&pool, "named").await?);
		let outsider = user(insert_user(&pool, "outsider").await?);
		let team: Uuid = sqlx::query_scalar("INSERT INTO teams (name) VALUES ('HR') RETURNING id").fetch_one(&pool).await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)")
			.bind(team)
			.bind(member.user_id)
			.execute(&pool)
			.await?;
		let role: Uuid = sqlx::query_scalar("INSERT INTO roles (name) VALUES ('hr-readers') RETURNING id")
			.fetch_one(&pool)
			.await?;
		sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
			.bind(holder.user_id)
			.bind(role)
			.execute(&pool)
			.await?;
		let base = KnowledgeBase::find(&pool, &fixture.admin, &fixture.base).await?.ok_or("missing")?;

		let shares = targets(&[
			(AssignmentKind::Team, Some(team), Some(AssignmentRole::Editor)),
			(AssignmentKind::Role, Some(role), None),
			(AssignmentKind::User, Some(named.user_id), None),
		]);
		base.replace_assignments(&pool, &fixture.admin, &shares).await?;
		for actor in [&member, &holder, &named] {
			assert_eq!(visible(&pool, actor).await?, [fixture.base]);
		}
		assert!(visible(&pool, &outsider).await?.is_empty());
		let as_member = KnowledgeBase::find(&pool, &member, &fixture.base).await?.ok_or("missing")?;
		let as_holder = KnowledgeBase::find(&pool, &holder, &fixture.base).await?.ok_or("missing")?;
		assert!(member.can_edit(&as_member) && !holder.can_edit(&as_holder));

		let assignments = base.assignments(&pool, &fixture.admin).await?;
		let kinds: Vec<AssignmentKind> = assignments.targets.iter().map(|target| target.kind).collect();
		assert_eq!(kinds, [AssignmentKind::Role, AssignmentKind::Team, AssignmentKind::User]);
		assert!(base.assignments(&pool, &member).await?.kinds.is_empty());

		base.replace_assignments(&pool, &fixture.admin, &targets(&[(AssignmentKind::Everyone, None, None)]))
			.await?;
		assert_eq!(visible(&pool, &outsider).await?, [fixture.base]);
		let as_member = KnowledgeBase::find(&pool, &member, &fixture.base).await?.ok_or("missing")?;
		assert!(!member.can_edit(&as_member));
		let events: Vec<String> = sqlx::query_scalar("SELECT event FROM audit_logs WHERE target_type = 'knowledge_base' ORDER BY created_at, event")
			.fetch_all(&pool)
			.await?;
		assert_eq!(events, ["knowledge_base_created", "knowledge_base_sharing_updated", "knowledge_base_sharing_updated"]);
		Ok(())
	}

	#[sqlx::test]
	async fn only_knowledge_admins_share_and_only_with_valid_targets(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let base = KnowledgeBase::find(&pool, &fixture.admin, &fixture.base).await?.ok_or("missing")?;
		let everyone = targets(&[(AssignmentKind::Everyone, None, None)]);
		assert!(matches!(
			base.replace_assignments(&pool, &user(Uuid::new_v4()), &everyone).await,
			Err(RagError::Forbidden)
		));
		for invalid in [
			targets(&[(AssignmentKind::Key, Some(Uuid::new_v4()), None)]),
			targets(&[(AssignmentKind::Everyone, None, Some(AssignmentRole::Editor))]),
			targets(&[(AssignmentKind::Team, None, None)]),
			targets(&[(AssignmentKind::Team, Some(Uuid::new_v4()), None)]),
		] {
			assert!(matches!(
				base.replace_assignments(&pool, &fixture.admin, &invalid).await,
				Err(RagError::InvalidAssignment)
			));
		}
		Ok(())
	}

	#[sqlx::test]
	async fn editors_change_the_name_but_not_the_source(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let editor = user(insert_user(&pool, "editor").await?);
		let base = KnowledgeBase::find(&pool, &fixture.admin, &fixture.base).await?.ok_or("missing")?;
		base.replace_assignments(
			&pool,
			&fixture.admin,
			&targets(&[(AssignmentKind::User, Some(editor.user_id), Some(AssignmentRole::Editor))]),
		)
		.await?;
		let base = KnowledgeBase::find(&pool, &editor, &fixture.base).await?.ok_or("missing")?;
		let mut rename = SaveKnowledgeBaseRequest {
			name: "Handbook".to_string(),
			description: Some("For everyone in HR".to_string()),
			..Default::default()
		};
		base.update(&pool, &editor, &rename, None).await?;
		rename.view = base.external_view().cloned();
		assert!(matches!(base.update(&pool, &editor, &rename, rename.view.as_ref()).await, Err(RagError::Forbidden)));
		assert!(matches!(base.delete(&pool, &editor).await, Err(RagError::Forbidden)));
		let renamed = KnowledgeBase::find(&pool, &editor, &fixture.base).await?.ok_or("missing")?;
		assert_eq!((renamed.name.as_str(), renamed.description.as_deref()), ("Handbook", Some("For everyone in HR")));
		assert!(editor.can_search(&renamed));
		Ok(())
	}

	#[sqlx::test]
	async fn labels_are_held_through_teams_roles_and_users_and_checked_live(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let member = insert_user(&pool, "member").await?;
		let holder = insert_user(&pool, "holder").await?;
		let named = insert_user(&pool, "named").await?;
		let outsider = insert_user(&pool, "outsider").await?;
		let team: Uuid = sqlx::query_scalar("INSERT INTO teams (name) VALUES ('HR') RETURNING id").fetch_one(&pool).await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)")
			.bind(team)
			.bind(member)
			.execute(&pool)
			.await?;
		let role: Uuid = sqlx::query_scalar("INSERT INTO roles (name) VALUES ('engineers') RETURNING id").fetch_one(&pool).await?;
		sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
			.bind(holder)
			.bind(role)
			.execute(&pool)
			.await?;
		let base = KnowledgeBase::find(&pool, &fixture.admin, &fixture.base).await?.ok_or("missing")?;
		let map = |label: &'static str, kind, id| {
			let base = base.clone();
			let pool = pool.clone();
			let admin = fixture.admin.clone();
			async move { base.replace_label_assignments(&pool, &admin, label, &targets(&[(kind, Some(id), None)])).await }
		};

		let refused = base.replace_label_assignments(&pool, &user(member), "HR Members", &targets(&[(AssignmentKind::Team, Some(team), None)])).await;
		assert!(matches!(refused, Err(RagError::Forbidden)));
		assert!(matches!(map("", AssignmentKind::Team, team).await, Err(RagError::InvalidLabel)));
		let everyone = base.replace_label_assignments(&pool, &fixture.admin, "HR Members", &targets(&[(AssignmentKind::Everyone, None, None)])).await;
		assert!(matches!(everyone, Err(RagError::InvalidAssignment)));
		assert!(matches!(map("HR Members", AssignmentKind::Team, Uuid::new_v4()).await, Err(RagError::InvalidAssignment)));

		map("HR Members", AssignmentKind::Team, team).await?;
		map("space:ENG", AssignmentKind::Role, role).await?;
		map("site-42/owners", AssignmentKind::User, named).await?;
		assert!(base.held_labels(&pool, &member).await?.is_empty(), "labels only count with per-record access");

		sqlx::query("UPDATE knowledge_bases SET view = jsonb_set(view, '{access}', '\"oxide_access\"') WHERE id = $1")
			.bind(base.id)
			.execute(&pool)
			.await?;
		let base = KnowledgeBase::find(&pool, &fixture.admin, &fixture.base).await?.ok_or("missing")?;
		assert_eq!(base.held_labels(&pool, &member).await?, ["HR Members"]);
		assert_eq!(base.held_labels(&pool, &holder).await?, ["space:ENG"]);
		assert_eq!(base.held_labels(&pool, &named).await?, ["site-42/owners"]);
		assert!(base.held_labels(&pool, &outsider).await?.is_empty());

		sqlx::query("DELETE FROM team_members WHERE user_id = $1").bind(member).execute(&pool).await?;
		assert!(base.held_labels(&pool, &member).await?.is_empty());

		let holders = base.label_assignments(&pool, &fixture.admin, "space:ENG").await?;
		assert_eq!(holders.targets.iter().map(|target| target.id).collect::<Vec<_>>(), [Some(role)]);
		assert!(base.label_assignments(&pool, &user(member), "space:ENG").await?.kinds.is_empty());
		base.replace_label_assignments(&pool, &fixture.admin, "space:ENG", &targets(&[])).await?;
		assert!(base.held_labels(&pool, &holder).await?.is_empty());

		let events: Vec<String> = sqlx::query_scalar("SELECT event FROM audit_logs WHERE event = 'knowledge_base_access_label_updated'")
			.fetch_all(&pool)
			.await?;
		assert_eq!(events.len(), 4);
		Ok(())
	}

	#[sqlx::test]
	async fn a_connection_in_use_cannot_be_deleted(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		assert!(matches!(
			RagConnection::delete(&pool, &fixture.admin.user_id, &fixture.connection.id).await,
			Err(RagError::ConnectionInUse)
		));
		let base = KnowledgeBase::find(&pool, &fixture.admin, &fixture.base).await?.ok_or("missing")?;
		base.delete(&pool, &fixture.admin).await?;
		assert!(RagConnection::delete(&pool, &fixture.admin.user_id, &fixture.connection.id).await?);
		Ok(())
	}
}

#[cfg(test)]
mod pgvector {
	use super::views::view;
	use crate::tests::support::TestResult;
	use crate::types::rag::{Distance, FieldFilter, StoreError, VectorStore, parse_version};
	use serde_json::json;
	use sqlx::PgPool;
	use std::borrow::Cow;

	/// Creates the collection and returns its vector columns; `small` (halfvec) only where pgvector has it (0.7+, as in CI).
	async fn collection(pool: &PgPool) -> TestResult<Vec<&'static str>> {
		sqlx::query("CREATE EXTENSION IF NOT EXISTS vector").execute(pool).await?;
		let halfvec: bool = sqlx::query_scalar("SELECT to_regtype('halfvec') IS NOT NULL").fetch_one(pool).await?;
		sqlx::query("CREATE SCHEMA docs").execute(pool).await?;
		sqlx::query("CREATE TABLE docs.\"Chunks\" (id serial, page_content text, site text, metadata jsonb, embedding vector(3))")
			.execute(pool)
			.await?;
		sqlx::query(
			"INSERT INTO docs.\"Chunks\" (page_content, site, metadata, embedding) VALUES
			 ('Holiday rules', 'HR', '{\"title\": \"Holidays\", \"url\": \"https://intranet.example/holidays\"}', '[1,0,0]'),
			 ('Expense rules', 'HR', '{\"title\": \"Expenses\"}', '[0,1,0]'),
			 ('Server setup', 'IT', '{\"title\": \"Servers\"}', '[0.9,0.1,0]')",
		)
		.execute(pool)
		.await?;
		if !halfvec {
			return Ok(vec!["embedding"]);
		}
		sqlx::query("ALTER TABLE docs.\"Chunks\" ADD COLUMN small halfvec(3)").execute(pool).await?;
		sqlx::query("UPDATE docs.\"Chunks\" SET small = embedding::halfvec").execute(pool).await?;
		Ok(vec!["embedding", "small"])
	}

	#[sqlx::test]
	async fn describe_lists_vectors_fields_and_a_sample(pool: PgPool) -> TestResult {
		let columns = collection(&pool).await?;
		let store = VectorStore::Pgvector(Cow::Borrowed(&pool));
		let info = store.describe("docs.\"Chunks\"", &[]).await?;
		let vectors: Vec<(Option<&str>, Option<u32>)> = info.vectors.iter().map(|vector| (vector.name.as_deref(), vector.size)).collect();
		let expected: Vec<(Option<&str>, Option<u32>)> = columns.into_iter().map(|column| (Some(column), Some(3))).collect();
		assert_eq!(vectors, expected);
		assert!(info.fields.contains(&"metadata.title".to_string()) && info.fields.contains(&"site".to_string()));
		assert!(!info.fields.contains(&"embedding".to_string()));
		assert_eq!(info.sample.len(), 3);
		assert!(info.sample[0].get("embedding").is_none());
		assert_eq!(store.describe("docs.missing", &[]).await.err(), Some(StoreError::CollectionNotFound));
		let unknown = [FieldFilter {
			field: "nope".to_string(),
			value: json!("HR"),
		}];
		assert_eq!(store.describe("docs.\"Chunks\"", &unknown).await.err(), Some(StoreError::UnknownField));
		let nested = [FieldFilter {
			field: "metadata.title".to_string(),
			value: json!("Holidays"),
		}];
		assert_eq!(store.describe("docs.\"Chunks\"", &nested).await?.sample.len(), 1);
		let numeric = [FieldFilter {
			field: "id".to_string(),
			value: json!(2.0),
		}];
		assert_eq!(store.describe("docs.\"Chunks\"", &numeric).await?.sample.len(), 1);
		Ok(())
	}

	#[sqlx::test]
	async fn search_ranks_nearest_first_inside_the_fixed_filter(pool: PgPool) -> TestResult {
		let columns = collection(&pool).await?;
		let store = VectorStore::Pgvector(Cow::Borrowed(&pool));
		let version: String = sqlx::query_scalar("SELECT extversion FROM pg_extension WHERE extname = 'vector'")
			.fetch_one(&pool)
			.await?;
		if parse_version(&version) < (0, 8) {
			let hr = view("docs.\"Chunks\"", Some("embedding"));
			assert_eq!(store.search(&hr, &[1.0, 0.0, 0.0], 5, &[]).await.err(), Some(StoreError::ExtensionTooOld));
			return Ok(());
		}
		for column in columns {
			let mut hr = view("docs.\"Chunks\"", Some(column));
			hr.fields.text = "page_content".to_string();
			hr.filter = vec![FieldFilter {
				field: "site".to_string(),
				value: json!("HR"),
			}];
			let records = store.search(&hr, &[1.0, 0.05, 0.0], 5, &[]).await?;
			let hits: Vec<String> = records.iter().filter_map(|(payload, score)| hr.hit(payload, *score)).map(|hit| hit.text).collect();
			assert_eq!(hits, ["Holiday rules", "Expense rules"], "{column}");
			assert!(records[0].1 > records[1].1);
			hr.distance = Distance::Euclid;
			assert_eq!(store.search(&hr, &[0.0, 1.0, 0.0], 1, &[]).await?.len(), 1);
		}
		sqlx::query("INSERT INTO docs.\"Chunks\" (page_content, site) VALUES ('Not embedded yet', 'HR')").execute(&pool).await?;
		let mut unembedded = view("docs.\"Chunks\"", Some("embedding"));
		unembedded.fields.text = "page_content".to_string();
		assert_eq!(store.search(&unembedded, &[1.0, 0.0, 0.0], 10, &[]).await?.len(), 3, "rows without a vector are skipped");
		assert_eq!(store.describe("pg_catalog.pg_authid", &[]).await.err(), Some(StoreError::CollectionNotFound));
		assert_eq!(store.describe("a.b.c.d", &[]).await.err(), Some(StoreError::CollectionNotFound));
		let mut wrong = view("docs.\"Chunks\"", Some("page_content"));
		wrong.fields.text = "page_content".to_string();
		assert_eq!(store.search(&wrong, &[1.0, 0.0, 0.0], 1, &[]).await.err(), Some(StoreError::UnknownField));
		Ok(())
	}
	/// Records list who may see them in a `text[]` column and, nested, in a JSON column; one record lists nobody.
	async fn labelled(pool: &PgPool) -> TestResult<bool> {
		sqlx::query("CREATE EXTENSION IF NOT EXISTS vector").execute(pool).await?;
		let version: String = sqlx::query_scalar("SELECT extversion FROM pg_extension WHERE extname = 'vector'")
			.fetch_one(pool)
			.await?;
		sqlx::query("CREATE TABLE acl (page_content text, site text, oxide_access text[], metadata jsonb, embedding vector(3))")
			.execute(pool)
			.await?;
		sqlx::query(
			"INSERT INTO acl VALUES
			 ('HR only', 'intranet', ARRAY['HR Members'], '{\"acl\": {\"groups\": [\"HR Members\"]}}', '[1,0,0]'),
			 ('Engineering', 'intranet', ARRAY['space:ENG', 'site-42/owners'], '{\"acl\": {\"groups\": \"space:ENG\"}}', '[0.9,0.1,0]'),
			 ('Nobody listed', 'intranet', NULL, '{}', '[1,0,0]'),
			 ('Other site', 'extranet', ARRAY['HR Members'], '{\"acl\": {\"groups\": [\"HR Members\"]}}', '[1,0,0]')",
		)
		.execute(pool)
		.await?;
		sqlx::query("ALTER TABLE acl ADD COLUMN groups varchar(64)[]").execute(pool).await?;
		sqlx::query("UPDATE acl SET groups = oxide_access::varchar(64)[]").execute(pool).await?;
		Ok(parse_version(&version) >= (0, 8))
	}

	#[sqlx::test]
	async fn search_only_finds_records_listing_a_held_label(pool: PgPool) -> TestResult {
		if !labelled(&pool).await? {
			return Ok(());
		}
		let store = VectorStore::Pgvector(Cow::Borrowed(&pool));
		for field in ["oxide_access", "metadata.acl.groups", "groups"] {
			let mut acl = view("acl", Some("embedding"));
			acl.filter = vec![FieldFilter {
				field: "site".to_string(),
				value: json!("intranet"),
			}];
			acl.access = Some(field.to_string());
			let texts = |records: Vec<(serde_json::Value, f64)>| -> Vec<String> {
				records.iter().filter_map(|(payload, score)| acl.hit(payload, *score)).map(|hit| hit.text).collect()
			};
			let hr = vec!["HR Members".to_string()];
			assert_eq!(texts(store.search(&acl, &[1.0, 0.0, 0.0], 10, &hr).await?), ["HR only"], "{field}");
			let both = vec!["HR Members".to_string(), "space:ENG".to_string()];
			assert_eq!(texts(store.search(&acl, &[1.0, 0.0, 0.0], 10, &both).await?), ["HR only", "Engineering"], "{field}");
			assert!(store.search(&acl, &[1.0, 0.0, 0.0], 10, &[]).await?.is_empty(), "{field}");
			let stranger = vec!["Sales".to_string()];
			assert!(store.search(&acl, &[1.0, 0.0, 0.0], 10, &stranger).await?.is_empty(), "{field}");
		}
		let mut open = view("acl", Some("embedding"));
		open.filter = vec![FieldFilter {
			field: "site".to_string(),
			value: json!("intranet"),
		}];
		assert_eq!(store.search(&open, &[1.0, 0.0, 0.0], 10, &[]).await?.len(), 3);
		let mut unknown = open.clone();
		unknown.access = Some("missing".to_string());
		assert_eq!(store.search(&unknown, &[1.0, 0.0, 0.0], 10, &["x".to_string()]).await.err(), Some(StoreError::UnknownField));
		Ok(())
	}

	#[sqlx::test]
	async fn access_labels_are_discovered_inside_the_fixed_filter(pool: PgPool) -> TestResult {
		labelled(&pool).await?;
		let store = VectorStore::Pgvector(Cow::Borrowed(&pool));
		let mut acl = view("acl", Some("embedding"));
		assert!(store.access_labels(&acl).await?.is_empty());
		acl.access = Some("oxide_access".to_string());
		assert_eq!(store.access_labels(&acl).await?, ["HR Members", "site-42/owners", "space:ENG"]);
		acl.access = Some("metadata.acl.groups".to_string());
		acl.filter = vec![FieldFilter {
			field: "site".to_string(),
			value: json!("intranet"),
		}];
		assert_eq!(store.access_labels(&acl).await?, ["HR Members", "space:ENG"]);
		Ok(())
	}

	#[sqlx::test]
	async fn managed_pgvector_table_upserts_searches_updates_access_and_drops_old_generations(pool: PgPool) -> TestResult {
		use crate::types::rag::{ManagedSearch, ManagedVector, managed_table_name};
		use uuid::Uuid;

		sqlx::query("CREATE EXTENSION IF NOT EXISTS vector").execute(&pool).await?;
		let version: String = sqlx::query_scalar("SELECT extversion FROM pg_extension WHERE extname = 'vector'")
			.fetch_one(&pool)
			.await?;
		if parse_version(&version) < (0, 8) {
			return Ok(());
		}

		let store = VectorStore::Pgvector(Cow::Borrowed(&pool));
		let conn_id = Uuid::new_v4();
		let kb_a = Uuid::new_v4();
		let kb_b = Uuid::new_v4();
		let doc_open = Uuid::new_v4();
		let doc_hr = Uuid::new_v4();
		let doc_locked = Uuid::new_v4();
		let doc_other_kb = Uuid::new_v4();
		let (c_open, c_hr, c_locked, c_other) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());

		store.ensure_managed(&conn_id, 1, 3).await?;
		store
			.upsert_managed(
				&conn_id,
				1,
				&[
					ManagedVector {
						chunk_id: c_open,
						knowledge_base_id: kb_a,
						document_id: doc_open,
						access: None,
						embedding: vec![1.0, 0.0, 0.0],
					},
					ManagedVector {
						chunk_id: c_hr,
						knowledge_base_id: kb_a,
						document_id: doc_hr,
						access: Some(vec!["team:hr".to_string()]),
						embedding: vec![0.9, 0.1, 0.0],
					},
					ManagedVector {
						chunk_id: c_locked,
						knowledge_base_id: kb_a,
						document_id: doc_locked,
						access: Some(Vec::new()),
						embedding: vec![1.0, 0.0, 0.0],
					},
					ManagedVector {
						chunk_id: c_other,
						knowledge_base_id: kb_b,
						document_id: doc_other_kb,
						access: None,
						embedding: vec![1.0, 0.0, 0.0],
					},
				],
			)
			.await?;

		fn search_req<'a>(connection_id: &'a Uuid, knowledge_base_id: &'a Uuid, labels: &'a [String]) -> ManagedSearch<'a> {
			ManagedSearch {
				connection_id,
				generation: 1,
				knowledge_base_id,
				vector: &[1.0, 0.0, 0.0],
				limit: 10,
				labels,
			}
		}
		let ids = |hits: Vec<(Uuid, f64)>| hits.into_iter().map(|(id, _)| id).collect::<Vec<_>>();

		assert_eq!(ids(store.search_managed(&search_req(&conn_id, &kb_a, &[])).await?), [c_open]);
		let hr_label = vec!["team:hr".to_string()];
		assert_eq!(ids(store.search_managed(&search_req(&conn_id, &kb_a, &hr_label)).await?), [c_open, c_hr]);

		let alice_label = vec!["user:alice".to_string()];
		store
			.update_managed_access(&conn_id, &doc_open, Some(alice_label.as_slice()))
			.await?;
		assert!(store.search_managed(&search_req(&conn_id, &kb_a, &[])).await?.is_empty());
		assert_eq!(ids(store.search_managed(&search_req(&conn_id, &kb_a, &alice_label)).await?), [c_open]);

		store.delete_managed_document(&conn_id, &doc_hr).await?;
		assert!(store.search_managed(&search_req(&conn_id, &kb_a, &hr_label)).await?.is_empty());

		store.ensure_managed(&conn_id, 2, 3).await?;
		store.drop_old_managed_tables(&conn_id, 1).await?;
		let newer_kept: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
			.bind(managed_table_name(&conn_id, 2))
			.fetch_one(&pool)
			.await?;
		assert!(newer_kept, "a worker with a stale generation must not drop a newer table");
		store.drop_old_managed_tables(&conn_id, 2).await?;
		let old_exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
			.bind(managed_table_name(&conn_id, 1))
			.fetch_one(&pool)
			.await?;
		let new_exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
			.bind(managed_table_name(&conn_id, 2))
			.fetch_one(&pool)
			.await?;
		assert!(!old_exists, "old generation table should be dropped");
		assert!(new_exists, "current generation table should stay");
		Ok(())
	}
}

#[cfg(test)]
mod retrieval {
	use super::validation::request;
	use super::views::view;
	use crate::tests::gateway::support::job_state;
	use crate::tests::support::{TestResult, create_user, insert_user};
	use crate::types::rag::{
		Candidate, ConnectionKind, ConnectionMode, KNOWLEDGE_BUILTIN, KNOWLEDGE_TOOL, KnowledgeActor, KnowledgeBase, KnowledgeHit, RagConnection,
		SaveKnowledgeBaseRequest, add_knowledge_tool, number, per_document, rrf, run_knowledge_tool, turn_knowledge_bases,
	};
	use crate::types::{AssignmentKind, AssignmentRef, ReplaceAssignmentsRequest, Tool};
	use omniference::types::{ChatRequestIR, ToolChoice, ToolSpec};
	use serde_json::json;
	use sqlx::PgPool;
	use uuid::Uuid;

	fn candidate(base: Uuid, text: &str, url: Option<&str>) -> Candidate {
		Candidate {
			knowledge_base: base,
			knowledge_base_name: "Handbook".to_string(),
			document_id: None,
			hit: KnowledgeHit {
				text: text.to_string(),
				title: None,
				url: url.map(str::to_string),
				section: None,
				page: None,
				score: 0.0,
			},
		}
	}

	fn texts(candidates: &[Candidate]) -> Vec<&str> {
		candidates.iter().map(|candidate| candidate.hit.text.as_str()).collect()
	}

	#[test]
	fn rrf_rewards_hits_found_by_several_bases_and_drops_duplicates() {
		let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
		let merged = rrf(vec![
			vec![candidate(first, "a", None), candidate(first, "shared", None), candidate(first, "b", None)],
			vec![candidate(second, "c", None), candidate(second, "shared ", None)],
		]);
		assert_eq!(merged.first().map(|candidate| candidate.hit.text.trim()), Some("shared"));
		assert_eq!(merged.len(), 4);
	}

	#[test]
	fn at_most_three_excerpts_come_from_one_document() {
		let base = Uuid::new_v4();
		let doc = Some("https://intranet.example/a");
		let list = vec![
			candidate(base, "1", doc),
			candidate(base, "2", doc),
			candidate(base, "3", doc),
			candidate(base, "4", doc),
			candidate(base, "5", Some("https://intranet.example/b")),
		];
		assert_eq!(texts(&per_document(list)), ["1", "2", "3", "5"]);
	}

	#[test]
	fn sources_are_numbered_and_capped() {
		let base = Uuid::new_v4();
		let long = "x".repeat(9_000);
		let sources = number(vec![candidate(base, &long, None), candidate(base, &long, None), candidate(base, "short", None)]);
		assert_eq!(sources.len(), 3);
		assert_eq!(sources.iter().map(|source| source.n).collect::<Vec<_>>(), [1, 2, 3]);
		assert_eq!(sources[0].text.chars().count(), 4_000);
		let many: Vec<Candidate> = (0..10).map(|_| candidate(base, &long, None)).collect();
		assert_eq!(number(many).len(), 4);
	}

	async fn shared_base(pool: &PgPool, name: &str) -> TestResult<(KnowledgeBase, KnowledgeActor)> {
		crate::utils::encryption::init_for_test([0x42; 32])?;
		let admin = KnowledgeActor::basic(create_user(pool).await?, true, true);
		let connection = match RagConnection::list(pool).await?.into_iter().next() {
			Some(connection) => connection,
			None => {
				let request = request(ConnectionKind::Qdrant, ConnectionMode::ReadOnly, Some("https://qdrant.example.com"), None);
				RagConnection::create(pool, &admin.user_id, &request, None).await?
			}
		};
		let mut external = view("docs", None);
		external.connection_id = connection.id;
		let save = SaveKnowledgeBaseRequest {
			name: name.to_string(),
			description: Some("HR policies".to_string()),
			view: Some(external.clone()),
			..Default::default()
		};
		let id = KnowledgeBase::create(pool, &admin, &save, &external).await?;
		Ok((KnowledgeBase::find(pool, &admin, &id).await?.ok_or("missing")?, admin))
	}

	async fn enable(pool: &PgPool, enabled: bool) -> TestResult {
		let updated = sqlx::query("UPDATE tools SET is_enabled = $1 WHERE name = 'knowledge_search' AND owner_id IS NULL").bind(enabled).execute(pool).await?;
		if updated.rows_affected() == 0 {
			sqlx::query("INSERT INTO tools (name, display_name, source_kind, source_config, input_schema, is_enabled) VALUES ('knowledge_search', 'Knowledge Search', 'BUILTIN', $1, '{}', $2)")
				.bind(json!({"builtin_id": KNOWLEDGE_BUILTIN}))
				.bind(enabled)
				.execute(pool)
				.await?;
		}
		Ok(())
	}

	#[sqlx::test]
	async fn only_shared_bases_of_the_turn_are_searched_while_knowledge_is_on(pool: PgPool) -> TestResult {
		let (base, admin) = shared_base(&pool, "Handbook").await?;
		let member = insert_user(&pool, "member").await?;
		let state = job_state(pool.clone());

		enable(&pool, true).await?;
		assert!(turn_knowledge_bases(&state, &member, &[base.id]).await?.is_empty());
		assert!(turn_knowledge_bases(&state, &member, &[]).await?.is_empty());
		let unshared = run_knowledge_tool(&state, &member, None, &[base.id], &json!({"query": "holidays"})).await?;
		assert!(unshared.sources.is_empty() && unshared.unavailable.is_empty());

		let everyone = ReplaceAssignmentsRequest {
			targets: vec![AssignmentRef {
				kind: AssignmentKind::Everyone,
				id: None,
				role: None,
			}],
		};
		base.replace_assignments(&pool, &admin, &everyone).await?;
		assert_eq!(turn_knowledge_bases(&state, &member, &[base.id, Uuid::new_v4()]).await?.len(), 1);
		let broken = run_knowledge_tool(&state, &member, None, &[base.id], &json!({"query": "holidays"})).await?;
		assert!(broken.sources.is_empty());
		assert_eq!(broken.unavailable, ["Handbook"]);
		let other = run_knowledge_tool(&state, &member, None, &[base.id], &json!({"query": "holidays", "knowledge_base": "Elsewhere"})).await?;
		assert!(other.sources.is_empty());
		assert!(run_knowledge_tool(&state, &member, None, &[base.id], &json!({"query": " "})).await.is_err());

		enable(&pool, false).await?;
		assert!(turn_knowledge_bases(&state, &member, &[base.id]).await?.is_empty());
		Ok(())
	}

	#[sqlx::test]
	async fn chats_keep_only_bases_their_user_may_search(pool: PgPool) -> TestResult {
		let (shared, admin) = shared_base(&pool, "Shared").await?;
		let (hidden, _) = shared_base(&pool, "Hidden").await?;
		let member = insert_user(&pool, "member").await?;
		let everyone = ReplaceAssignmentsRequest {
			targets: vec![AssignmentRef {
				kind: AssignmentKind::Everyone,
				id: None,
				role: None,
			}],
		};
		shared.replace_assignments(&pool, &admin, &everyone).await?;
		let chat_id: Uuid = sqlx::query_scalar("INSERT INTO chats (user_id) VALUES ($1) RETURNING id")
			.bind(member)
			.fetch_one(&pool)
			.await?;
		let kept = KnowledgeBase::attach_to_chat(&pool, &member, &chat_id, &[shared.id, hidden.id]).await?;
		assert_eq!(kept, [shared.id]);
		assert_eq!(KnowledgeBase::chat_ids(&pool, &chat_id).await?, [shared.id]);
		assert!(KnowledgeBase::attach_to_chat(&pool, &member, &chat_id, &[]).await?.is_empty());
		assert!(KnowledgeBase::chat_ids(&pool, &chat_id).await?.is_empty());
		Ok(())
	}

	#[sqlx::test]
	async fn the_tool_names_the_attached_bases(pool: PgPool) -> TestResult {
		let (base, _) = shared_base(&pool, "Handbook").await?;
		let mut ir = ChatRequestIR::default();
		add_knowledge_tool(&mut ir, &[]);
		assert!(ir.tools.is_empty());
		ir.tools.push(ToolSpec::JsonSchema {
			name: KNOWLEDGE_TOOL.to_string(),
			description: Some("someone else's tool".to_string()),
			schema: json!({"type": "object"}),
			strict: None,
			defer_loading: false,
		});
		add_knowledge_tool(&mut ir, std::slice::from_ref(&base));
		assert_eq!(ir.tools.len(), 1);
		let ToolSpec::JsonSchema { name, description, schema, .. } = &ir.tools[0];
		assert_eq!(name, KNOWLEDGE_TOOL);
		assert!(description.as_deref().is_some_and(|text| text.contains("- Handbook: HR policies")));
		assert_eq!(schema["properties"]["knowledge_base"]["enum"], json!(["Handbook"]));
		add_knowledge_tool(&mut ir, &[]);
		assert!(ir.tools.is_empty());
		assert!(matches!(ir.tool_choice, ToolChoice::None));
		Ok(())
	}

	#[sqlx::test]
	async fn only_an_enabled_system_knowledge_tool_turns_knowledge_on(pool: PgPool) -> TestResult {
		let owner = create_user(&pool).await?;
		let insert = "INSERT INTO tools (owner_id, name, display_name, source_kind, source_config, input_schema, is_enabled)
			VALUES ($1, $2, $2, 'BUILTIN', $3, '{}', $4) RETURNING id";
		let system: Uuid = sqlx::query_scalar(insert).bind(None::<Uuid>).bind("knowledge_search").bind(json!({"builtin_id": KNOWLEDGE_BUILTIN})).bind(true).fetch_one(&pool).await?;
		let personal: Uuid = sqlx::query_scalar(insert).bind(owner).bind("knowledge_search").bind(json!({"builtin_id": KNOWLEDGE_BUILTIN})).bind(true).fetch_one(&pool).await?;
		let other: Uuid = sqlx::query_scalar(insert).bind(None::<Uuid>).bind("websearch").bind(json!({"builtin_id": "websearch"})).bind(true).fetch_one(&pool).await?;
		assert_eq!(Tool::enabled_builtin_among(&pool, &[other, system], KNOWLEDGE_BUILTIN).await?, Some(system));
		assert_eq!(Tool::enabled_builtin_among(&pool, &[other, personal], KNOWLEDGE_BUILTIN).await?, None);
		sqlx::query("UPDATE tools SET is_enabled = false WHERE id = $1").bind(system).execute(&pool).await?;
		assert_eq!(Tool::enabled_builtin_among(&pool, &[system], KNOWLEDGE_BUILTIN).await?, None);
		Ok(())
	}
	#[sqlx::test]
	async fn presets_keep_only_bases_their_editor_may_search(pool: PgPool) -> TestResult {
		use crate::tests::gateway::support::{create_provider_model, create_team};
		use crate::types::{Preset, PresetActor, PresetResponse};
		let (shared, admin) = shared_base(&pool, "Shared").await?;
		let (hidden, _) = shared_base(&pool, "Hidden").await?;
		let everyone = ReplaceAssignmentsRequest {
			targets: vec![AssignmentRef {
				kind: AssignmentKind::Everyone,
				id: None,
				role: None,
			}],
		};
		shared.replace_assignments(&pool, &admin, &everyone).await?;
		let user_id = create_user(&pool).await?;
		create_team(&pool, user_id, "Knowledge team", true).await?;
		let (_, model_id) = create_provider_model(&pool, "knowledge-provider", "base").await?;
		let actor = PresetActor::load(&pool, &user_id, true, false).await?;
		let mut request = super::super::presets::validation::request("with-knowledge");
		request.base_model_id = model_id;
		request.knowledge_base_ids = Some(vec![shared.id, hidden.id]);
		let preset = Preset::create(&pool, &actor, request).await?;
		assert_eq!(KnowledgeBase::preset_ids(&pool, &preset.id).await?, [shared.id]);
		let responses: Vec<PresetResponse> = Preset::responses_for_user(&pool, &actor, vec![preset]).await?;
		assert_eq!(responses[0].knowledge_base_ids, [shared.id]);
		Ok(())
	}
}

#[cfg(test)]
mod model_access {
	use super::validation::request;
	use super::views::view;
	use crate::tests::gateway::support::job_state;
	use crate::tests::support::{TestResult, create_user};
	use crate::types::chat::runner::prepare::{allowed_for_model, require_knowledge_access};
	use crate::types::providers::ProviderKind;
	use crate::types::rag::{
		ConnectionKind, ConnectionMode, FieldFilter, KNOWLEDGE_CHAT_METADATA_KEY, KnowledgeActor, KnowledgeBase, ModelAccess, ModelTargets, RagConnection, RagError,
		RoutedModel, SaveKnowledgeBaseRequest, chat_allows_model, check_view,
	};
	use omniference::types::ProviderRouting;
	use sqlx::PgPool;
	use uuid::Uuid;

	fn routed(kind: ProviderKind) -> RoutedModel {
		RoutedModel {
			id: Uuid::new_v4(),
			provider_id: Uuid::new_v4(),
			kind,
		}
	}

	fn slugs(list: &[&str]) -> Vec<String> {
		list.iter().map(|slug| (*slug).to_string()).collect()
	}

	#[test]
	fn listed_models_and_providers_are_allowed_and_routing_providers_narrowed() {
		let direct = routed(ProviderKind::Openai);
		let router = routed(ProviderKind::Openrouter);
		assert_eq!(ModelTargets::default().decide(&direct), ModelAccess::Allowed);

		let by_model = ModelTargets { models: vec![direct.id], ..Default::default() };
		assert_eq!(by_model.decide(&direct), ModelAccess::Allowed);
		assert_eq!(by_model.decide(&router), ModelAccess::Denied);

		let by_provider = ModelTargets { providers: vec![direct.provider_id, router.provider_id], ..Default::default() };
		assert_eq!(by_provider.decide(&direct), ModelAccess::Allowed);
		assert_eq!(by_provider.decide(&router), ModelAccess::Allowed);

		let upstream = ModelTargets {
			providers: vec![router.provider_id],
			upstream_providers: slugs(&["anthropic"]),
			..Default::default()
		};
		assert_eq!(upstream.decide(&router), ModelAccess::Upstream(slugs(&["anthropic"])));
		assert_eq!(upstream.decide(&direct), ModelAccess::Denied);
	}

	#[test]
	fn a_listed_routing_model_is_still_narrowed_and_a_deleted_target_denies_everything() {
		let router = routed(ProviderKind::Openrouter);
		let direct = routed(ProviderKind::Openai);
		let listed = ModelTargets {
			models: vec![router.id],
			upstream_providers: slugs(&["azure"]),
			..Default::default()
		};
		assert_eq!(listed.decide(&router), ModelAccess::Upstream(slugs(&["azure"])));
		let removed = ModelTargets {
			providers: vec![direct.provider_id],
			removed: true,
			..Default::default()
		};
		assert_eq!(removed.decide(&direct), ModelAccess::Denied);
		assert!(!removed.is_unrestricted());
	}

	#[test]
	fn narrowing_drops_an_order_left_empty_and_narrows_parent_slugs() {
		let order = |slugs_in_order: &[&str]| Some(ProviderRouting { order: Some(slugs(slugs_in_order)), ..Default::default() });
		let narrowed = crate::types::provider_routing::narrow(order(&["mistral"]), &slugs(&["azure"])).flatten();
		assert_eq!(narrowed.and_then(|routing| routing.order), None);
		let narrowed = crate::types::provider_routing::narrow(order(&["deepinfra", "azure"]), &slugs(&["deepinfra/fp8"])).flatten();
		assert_eq!(narrowed.and_then(|routing| routing.order), Some(slugs(&["deepinfra/fp8"])));
	}

	#[test]
	fn every_knowledge_base_must_allow_the_model() {
		let router = routed(ProviderKind::Openrouter);
		let anthropic = ModelTargets { providers: vec![router.provider_id], upstream_providers: slugs(&["anthropic", "azure"]), ..Default::default() };
		let azure = ModelTargets { providers: vec![router.provider_id], upstream_providers: slugs(&["azure/eu"]), ..Default::default() };
		assert_eq!(ModelTargets::decide_all([&anthropic, &azure], &router), ModelAccess::Upstream(slugs(&["azure/eu"])));
		let other = ModelTargets { providers: vec![router.provider_id], upstream_providers: slugs(&["mistral"]), ..Default::default() };
		let without_router = ModelTargets { upstream_providers: slugs(&["anthropic"]), ..Default::default() };
		assert_eq!(without_router.decide(&router), ModelAccess::Denied, "upstream providers only narrow a listed routing provider");
		assert_eq!(ModelTargets::decide_all([&anthropic, &other], &router), ModelAccess::Denied);
		assert_eq!(ModelTargets::decide_all([&ModelTargets::default()], &router), ModelAccess::Allowed);
		assert!(!ModelAccess::Upstream(slugs(&["azure"])).is_allowed());
	}

	#[test]
	fn targets_are_normalized_and_bounded() -> Result<(), RagError> {
		let id = Uuid::new_v4();
		let targets = ModelTargets {
			providers: vec![id, id],
			upstream_providers: slugs(&[" Azure ", "azure"]),
			..Default::default()
		};
		assert_eq!(targets.normalized()?, ModelTargets { providers: vec![id], upstream_providers: slugs(&["azure"]), ..Default::default() });
		let bad = ModelTargets { upstream_providers: slugs(&["no spaces"]), ..Default::default() };
		assert!(matches!(bad.normalized(), Err(RagError::InvalidModelAccess)));
		Ok(())
	}

	async fn provider_with_model(pool: &PgPool, kind: &str, model: &str) -> TestResult<(Uuid, Uuid)> {
		let provider: Uuid = sqlx::query_scalar("INSERT INTO providers (kind, name, base_url, is_enabled) VALUES ($1::provider_kind, $2, 'https://example.com', true) RETURNING id")
			.bind(kind)
			.bind(format!("{kind} {}", Uuid::new_v4()))
			.fetch_one(pool)
			.await?;
		let model: Uuid = sqlx::query_scalar("INSERT INTO models (provider_id, model_id, display_name, is_enabled) VALUES ($1, $2, $2, true) RETURNING id")
			.bind(provider)
			.bind(model)
			.fetch_one(pool)
			.await?;
		Ok((provider, model))
	}

	/// A knowledge base whose embedding model belongs to its own provider.
	async fn base(pool: &PgPool) -> TestResult<(KnowledgeBase, KnowledgeActor, Uuid)> {
		let admin = KnowledgeActor::basic(create_user(pool).await?, true, true);
		let connection = RagConnection::create(
			pool,
			&admin.user_id,
			&request(ConnectionKind::Qdrant, ConnectionMode::ReadOnly, Some("https://qdrant.example.com"), None),
			None,
		)
		.await?;
		let (embedding_provider, embedding_model) = provider_with_model(pool, "OPENAI", "text-embedding-3-small").await?;
		let mut external = view("docs", None);
		external.connection_id = connection.id;
		external.embedding.model_id = embedding_model;
		let save = SaveKnowledgeBaseRequest {
			name: "Contracts".to_string(),
			view: Some(external.clone()),
			..Default::default()
		};
		let id = KnowledgeBase::create(pool, &admin, &save, &external).await?;
		Ok((KnowledgeBase::find(pool, &admin, &id).await?.ok_or("missing")?, admin, embedding_provider))
	}

	#[sqlx::test]
	async fn saving_needs_the_embedding_model_and_known_targets(pool: PgPool) -> TestResult {
		let (base, admin, embedding_provider) = base(&pool).await?;
		let (_, chat_model) = provider_with_model(&pool, "ANTHROPIC", "claude").await?;

		let without_embedding = ModelTargets { models: vec![chat_model], ..Default::default() };
		assert!(matches!(base.replace_model_access(&pool, &admin, &without_embedding).await, Err(RagError::EmbeddingModelNotAllowed)));
		let unknown = ModelTargets { providers: vec![embedding_provider, Uuid::new_v4()], ..Default::default() };
		assert!(matches!(base.replace_model_access(&pool, &admin, &unknown).await, Err(RagError::InvalidModelAccess)));

		let viewer = KnowledgeActor::basic(create_user(&pool).await?, false, false);
		let targets = ModelTargets { providers: vec![embedding_provider], models: vec![chat_model], ..Default::default() };
		assert!(matches!(base.replace_model_access(&pool, &viewer, &targets).await, Err(RagError::Forbidden)));
		let editor = KnowledgeBase { editor: true, ..base.clone() };
		assert!(matches!(editor.replace_model_access(&pool, &viewer, &targets).await, Err(RagError::Forbidden)), "editors cannot lift the restriction");
		let upstream_only = ModelTargets { providers: vec![embedding_provider], upstream_providers: slugs(&["anthropic"]), ..Default::default() };
		assert!(matches!(base.replace_model_access(&pool, &admin, &upstream_only).await, Err(RagError::UpstreamNeedsProvider)));

		base.replace_model_access(&pool, &admin, &targets).await?;
		let stored = base.model_access(&pool, &admin).await?.targets;
		assert_eq!(stored.providers, [embedding_provider]);
		assert_eq!(stored.models, [chat_model]);
		let (other_provider, _) = provider_with_model(&pool, "MISTRAL", "mistral-small").await?;
		let everything = base.model_access(&pool, &admin).await?;
		assert!(everything.providers.iter().any(|provider| provider.id == other_provider), "knowledge admins pick from every provider");
		let named = editor.model_access(&pool, &viewer).await?;
		assert_eq!(named.providers.iter().map(|provider| provider.id).collect::<Vec<_>>(), [embedding_provider], "editors see only the listed providers");
		assert_eq!(named.models.iter().map(|model| model.id).collect::<Vec<_>>(), [chat_model]);
		let audited: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE event = 'knowledge_base_model_access_updated' AND target_id = $1")
			.bind(base.id)
			.fetch_one(&pool)
			.await?;
		assert_eq!(audited, 1);
		Ok(())
	}

	#[sqlx::test]
	async fn the_name_and_model_access_are_saved_together_or_not_at_all(pool: PgPool) -> TestResult {
		let (base, admin, embedding_provider) = base(&pool).await?;
		let (_, chat_model) = provider_with_model(&pool, "ANTHROPIC", "claude").await?;
		let save = |name: &str, targets: ModelTargets| SaveKnowledgeBaseRequest {
			name: name.to_string(),
			model_access: Some(targets),
			..Default::default()
		};
		let refused = save("Refused", ModelTargets { models: vec![chat_model], ..Default::default() });
		assert!(matches!(base.update(&pool, &admin, &refused, None).await, Err(RagError::EmbeddingModelNotAllowed)));
		let name: String = sqlx::query_scalar("SELECT name FROM knowledge_bases WHERE id = $1").bind(base.id).fetch_one(&pool).await?;
		assert_ne!(name, "Refused");
		assert!(base.model_access(&pool, &admin).await?.targets.is_unrestricted());

		let viewer = KnowledgeActor::basic(create_user(&pool).await?, false, false);
		let editor = KnowledgeBase { editor: true, ..base.clone() };
		let targets = ModelTargets { providers: vec![embedding_provider], models: vec![chat_model], ..Default::default() };
		assert!(matches!(editor.update(&pool, &viewer, &save("Editor", targets.clone()), None).await, Err(RagError::Forbidden)));

		base.update(&pool, &admin, &save("Saved", targets), None).await?;
		let stored = base.model_access(&pool, &admin).await?.targets;
		assert_eq!((stored.providers, stored.models), (vec![embedding_provider], vec![chat_model]));
		Ok(())
	}

	#[test]
	fn long_descriptions_are_refused_with_their_own_reason() {
		let request = SaveKnowledgeBaseRequest {
			name: "Docs".to_string(),
			description: Some("x".repeat(2001)),
			..Default::default()
		};
		assert!(matches!(request.validate(), Err(RagError::InvalidDescription)));
	}

	#[sqlx::test]
	async fn qdrant_views_refuse_decimal_filters(pool: PgPool) -> TestResult {
		let user = create_user(&pool).await?;
		let connection = RagConnection::create(&pool, &user, &request(ConnectionKind::Qdrant, ConnectionMode::ReadOnly, Some("https://qdrant.example.com"), None), None).await?;
		let mut decimal = view("docs", None);
		decimal.connection_id = connection.id;
		decimal.filter = vec![FieldFilter {
			field: "version".to_string(),
			value: serde_json::json!(2.1),
		}];
		let state = job_state(pool.clone());
		assert!(matches!(check_view(&state, &user, &decimal).await, Err(RagError::InvalidFilter)));
		Ok(())
	}

	#[sqlx::test]
	async fn a_deleted_model_or_provider_keeps_the_restriction(pool: PgPool) -> TestResult {
		let (base, admin, embedding_provider) = base(&pool).await?;
		let (chat_provider, chat_model) = provider_with_model(&pool, "ANTHROPIC", "claude").await?;
		let (_, other) = provider_with_model(&pool, "OPENAI", "gpt").await?;
		let targets = ModelTargets { providers: vec![embedding_provider], models: vec![chat_model], ..Default::default() };
		base.replace_model_access(&pool, &admin, &targets).await?;

		sqlx::query("DELETE FROM providers WHERE id = ANY($1)").bind([embedding_provider, chat_provider]).execute(&pool).await?;
		let stored = ModelTargets::load(&pool, &[base.id]).await?.remove(&base.id).ok_or("targets gone")?;
		assert!(!stored.is_unrestricted(), "a deleted target must not lift the restriction");
		let model = RoutedModel::find(&pool, &other).await?.ok_or("model missing")?;
		assert_eq!(stored.decide(&model), ModelAccess::Denied);
		assert!(base.model_access(&pool, &admin).await?.removed_targets);
		Ok(())
	}

	struct Capture(std::sync::mpsc::Sender<Option<ProviderRouting>>);

	#[async_trait::async_trait]
	impl omniference::middleware::RequestHandler for Capture {
		async fn handle(&self, request: omniference::types::ChatRequestIR, _cancel: tokio_util::sync::CancellationToken) -> anyhow::Result<omniference::middleware::ChatStream> {
			self.0.send(request.provider_routing)?;
			Ok(Box::new(futures_util::stream::empty()))
		}
	}

	async fn send(pool: &PgPool, chat_id: Uuid, model_id: Uuid) -> anyhow::Result<Option<ProviderRouting>> {
		use omniference::middleware::Middleware;
		let middleware = crate::types::ProviderRoutingMiddleware::new(job_state(pool.clone()));
		let (sender, receiver) = std::sync::mpsc::channel();
		let request = omniference::types::ChatRequestIR {
			metadata: [(KNOWLEDGE_CHAT_METADATA_KEY.to_string(), chat_id.to_string()), ("oxide_model_id".to_string(), model_id.to_string())].into(),
			..Default::default()
		};
		let _stream = middleware.handle(request, tokio_util::sync::CancellationToken::new(), &Capture(sender)).await?;
		Ok(receiver.recv()?)
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn turns_only_offer_bases_whose_upstream_lists_still_overlap(pool: PgPool) -> TestResult {
		crate::config::Config::init(&pool).await?;
		crate::i18n::I18n::init(&pool).await;
		let (first, admin, embedding_provider) = base(&pool).await?;
		let external = first.external_view().ok_or("view")?.clone();
		let save = SaveKnowledgeBaseRequest {
			name: "Policies".to_string(),
			view: Some(external.clone()),
			..Default::default()
		};
		let id = KnowledgeBase::create(&pool, &admin, &save, &external).await?;
		let second = KnowledgeBase::find(&pool, &admin, &id).await?.ok_or("missing")?;
		let (router, routed_model) = provider_with_model(&pool, "OPENROUTER", "anthropic/claude").await?;
		for (base, upstream) in [(&first, "azure"), (&second, "anthropic")] {
			let targets = ModelTargets {
				providers: vec![embedding_provider, router],
				upstream_providers: slugs(&[upstream]),
				..Default::default()
			};
			base.replace_model_access(&pool, &admin, &targets).await?;
		}
		let chat_id: Uuid = sqlx::query_scalar("INSERT INTO chats (user_id) VALUES ($1) RETURNING id")
			.bind(admin.user_id)
			.fetch_one(&pool)
			.await?;

		let check = require_knowledge_access(&pool, &admin.user_id, &chat_id, &routed_model, None).await.map_err(|error| format!("{error:?}"))?;
		let mut bases = vec![first.clone(), second.clone()];
		allowed_for_model(&pool, &check, &mut bases).await.map_err(|error| format!("{error:?}"))?;
		assert_eq!(bases.iter().map(|base| base.id).collect::<Vec<_>>(), [first.id], "two bases with disjoint upstream lists are never searched together");

		KnowledgeBase::record_chat_sources(&pool, &chat_id, &[first.id]).await?;
		let locked = |slug: &str| Some(ProviderRouting { only: Some(slugs(&[slug])), ..Default::default() });
		assert!(require_knowledge_access(&pool, &admin.user_id, &chat_id, &routed_model, locked("azure")).await.is_ok());
		assert!(require_knowledge_access(&pool, &admin.user_id, &chat_id, &routed_model, locked("anthropic")).await.is_err(), "refused before the user message is stored");
		let check = require_knowledge_access(&pool, &admin.user_id, &chat_id, &routed_model, None).await.map_err(|error| format!("{error:?}"))?;
		let mut bases = vec![second.clone()];
		allowed_for_model(&pool, &check, &mut bases).await.map_err(|error| format!("{error:?}"))?;
		assert!(bases.is_empty(), "the chat's excerpts count as well");
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_chat_holding_excerpts_only_reaches_allowed_models(pool: PgPool) -> TestResult {
		crate::config::Config::init(&pool).await?;
		crate::i18n::I18n::init(&pool).await;
		let (base, admin, embedding_provider) = base(&pool).await?;
		let (allowed_provider, allowed) = provider_with_model(&pool, "ANTHROPIC", "claude").await?;
		let (_, other) = provider_with_model(&pool, "OPENAI", "gpt").await?;
		let (router, routed_model) = provider_with_model(&pool, "OPENROUTER", "anthropic/claude").await?;
		let targets = ModelTargets {
			providers: vec![embedding_provider, allowed_provider, router],
			upstream_providers: slugs(&["anthropic"]),
			..Default::default()
		};
		base.replace_model_access(&pool, &admin, &targets).await?;
		let chat_id: Uuid = sqlx::query_scalar("INSERT INTO chats (user_id) VALUES ($1) RETURNING id")
			.bind(admin.user_id)
			.fetch_one(&pool)
			.await?;

		assert!(send(&pool, chat_id, other).await?.is_none(), "a chat without excerpts is not restricted");
		assert!(chat_allows_model(&pool, &chat_id, &other).await?);

		KnowledgeBase::record_chat_sources(&pool, &chat_id, &[base.id]).await?;
		assert!(send(&pool, chat_id, allowed).await?.is_none());
		let narrowed = send(&pool, chat_id, routed_model).await?.ok_or("expected routing")?;
		assert_eq!(narrowed.only, Some(slugs(&["anthropic"])));
		assert!(!chat_allows_model(&pool, &chat_id, &routed_model).await?, "image and embedding requests cannot narrow routing");

		let error = send(&pool, chat_id, other).await.err().ok_or("expected a refusal")?;
		use omniference::skins::SkinErrorHandler;
		let response = omniference::skins::OpenAIErrorHandler.handle_inference_error(&omniference::adapter::InferenceError::from_handler_error(error));
		assert_eq!(response.status(), axum::http::StatusCode::FORBIDDEN);
		let body: serde_json::Value = serde_json::from_slice(&axum::body::to_bytes(response.into_body(), 10_000).await?)?;
		assert_eq!(body["error"]["code"], "knowledge_model_not_allowed");
		assert!(!chat_allows_model(&pool, &chat_id, &other).await?);

		let branch: Uuid = sqlx::query_scalar("INSERT INTO chats (user_id) VALUES ($1) RETURNING id")
			.bind(admin.user_id)
			.fetch_one(&pool)
			.await?;
		sqlx::query("INSERT INTO chat_knowledge_bases (chat_id, knowledge_base_id) VALUES ($1, $2)")
			.bind(chat_id)
			.bind(base.id)
			.execute(&pool)
			.await?;
		KnowledgeBase::copy_chat_sources(&pool, &chat_id, &branch).await?;
		assert!(!chat_allows_model(&pool, &branch, &other).await?, "a branch keeps the restriction");
		assert_eq!(KnowledgeBase::chat_ids(&pool, &branch).await?, vec![base.id], "a branch keeps the attached bases");

		base.delete(&pool, &admin).await?;
		assert!(!chat_allows_model(&pool, &chat_id, &other).await?, "deleting the knowledge base keeps the restriction");
		Ok(())
	}
}

#[cfg(test)]
mod chunker_and_extract {
	use crate::types::rag::{ChunkConfig, chunk_document, has_headings, normalize_vector, sha256_hex, split_embedding_batches};
	use crate::utils::extract::{self, DocumentKind};

	fn sample_pdf(pages: &[&str]) -> Vec<u8> {
		let mut objects: Vec<String> = Vec::new();
		let mut kids = Vec::new();
		let font_id = 3 + pages.len() * 2;
		for (index, text) in pages.iter().enumerate() {
			let page_id = 3 + index * 2;
			let content_id = page_id + 1;
			kids.push(format!("{page_id} 0 R"));
			let escaped = text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
			let stream = format!("BT /F1 12 Tf 72 720 Td ({escaped}) Tj ET");
			objects.push(format!(
				"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {content_id} 0 R /Resources << /Font << /F1 {font_id} 0 R >> >> >>"
			));
			objects.push(format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()));
		}
		let mut all = vec![
			"<< /Type /Catalog /Pages 2 0 R >>".to_string(),
			format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), pages.len()),
		];
		all.extend(objects);
		all.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());

		let mut out = b"%PDF-1.4\n".to_vec();
		let mut offsets = vec![0usize];
		for (index, body) in all.iter().enumerate() {
			offsets.push(out.len());
			out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", index + 1).as_bytes());
		}
		let xref_offset = out.len();
		out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
		for offset in offsets.iter().skip(1) {
			out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
		}
		out.extend_from_slice(
			format!(
				"trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
				offsets.len()
			)
			.as_bytes(),
		);
		out
	}

	#[test]
	fn chunker_preserves_headings_overlap_pages_and_markdown_tables() {
		let doc = "<!-- page 1 -->\n# Handbook\n\n## Holidays\n\nHolidays are requested through the HR portal. Every employee receives thirty vacation days per year.\n\nCarryover days expire at the end of March unless approved in writing by your manager.\n\n<!-- page 2 -->\n# Expenses\n\n| Category | Limit | Notes |\n| --- | --- | --- |\n| Meals | 50 EUR | Receipts required for every claim |\n| Travel | 400 EUR | Economy class rail or airfare only |\n| Lodging | 180 EUR | Standard business hotel per night |\n| Supplies | 90 EUR | Pre-approval needed for hardware |\n| Software | 120 EUR | Annual team license reimbursed |\n";
		let config = ChunkConfig {
			target_tokens: 45,
			overlap_percent: 20,
			..ChunkConfig::default()
		};
		let chunks = chunk_document(doc, config);
		assert!(chunks.len() >= 4, "expected multiple chunks, got {}", chunks.len());
		assert_eq!(chunks[0].heading_path.as_deref(), Some("Handbook › Holidays"));
		assert_eq!(chunks[0].page, Some(1));
		assert!(chunks[0].embed_text("policy.md").starts_with("policy.md › Handbook › Holidays\n\n"));
		assert!(!chunks[0].content.contains("<!-- page"));

		let expense_chunks: Vec<_> = chunks
			.iter()
			.filter(|chunk| chunk.heading_path.as_deref() == Some("Expenses"))
			.collect();
		assert!(expense_chunks.len() >= 2, "expected table to split across chunks");
		for chunk in &expense_chunks {
			assert_eq!(chunk.page, Some(2));
			assert!(chunk.content.starts_with("| Category | Limit | Notes |\n| --- | --- | --- |"));
			assert!(!chunk.content.contains("Carryover days"), "new top-level section must not carry overlap");
		}
		assert_eq!(chunks[0].content_hash, sha256_hex(chunks[0].content.as_bytes()));
	}

	#[test]
	fn chunker_carries_overlap_within_same_section() {
		let para_a = "Alpha sentence one explains the deployment pipeline clearly. Alpha sentence two covers rollback triggers.";
		let para_b = "Beta sentence one explains monitoring alerts and on-call escalation. Beta sentence two covers incident review.";
		let doc = format!("# Ops\n\n{para_a}\n\n{para_b}");
		let chunks = chunk_document(
			&doc,
			ChunkConfig {
				target_tokens: 35,
				overlap_percent: 25,
				..ChunkConfig::default()
			},
		);
		assert!(chunks.len() >= 2);
		assert!(
			chunks[1].content.contains("rollback") || chunks[1].content.contains("Alpha"),
			"expected overlap from chunk 0 in chunk 1: {:?}",
			chunks[1].content
		);
	}

	#[test]
	fn chunker_keeps_code_comments_and_line_breaks() {
		assert!(has_headings("notes.md", "text/plain") && has_headings("page.html", "text/html"));
		assert!(!has_headings("deploy.sh", "text/plain") && !has_headings("report.pdf", "application/pdf"));

		let script = "# install\napt-get update\n# run\n./start.sh";
		let config = ChunkConfig {
			headings: false,
			..ChunkConfig::default()
		};
		let chunks = chunk_document(script, config);
		assert_eq!(chunks.len(), 1, "comments must not split a code file");
		assert_eq!(chunks[0].heading_path, None);
		assert_eq!(chunks[0].content, script);

		let rows: Vec<String> = (0..40).map(|row| format!("{row},alpha beta gamma,äöü")).collect();
		let csv = rows.join("\n");
		let small = ChunkConfig {
			target_tokens: 50,
			overlap_percent: 0,
			headings: false,
		};
		let chunks = chunk_document(&csv, small);
		assert!(chunks.len() > 1);
		for chunk in &chunks {
			assert!(chunk.content.lines().all(|line| rows.iter().any(|row| row == line)), "rows stay on their own lines: {:?}", chunk.content);
		}
	}

	#[test]
	fn extract_detects_kinds_and_parses_text_html_and_pdf() {
		assert_eq!(DocumentKind::detect("notes.md", b"# Hi"), Some(DocumentKind::Text));
		assert_eq!(DocumentKind::detect("page.HTML", b"<h1>Hi</h1>"), Some(DocumentKind::Html));
		assert_eq!(DocumentKind::detect("doc.pdf", b"%PDF-1.4\n"), Some(DocumentKind::Pdf));
		assert_eq!(DocumentKind::detect("doc.pdf", b"not a pdf"), None);
		assert_eq!(DocumentKind::detect("bin.exe", b"MZ"), None);

		assert_eq!(extract::parse(DocumentKind::Text, b"\xEF\xBB\xBFhello\r\nworld").as_deref(), Some("hello\nworld"));
		assert_eq!(extract::parse(DocumentKind::Text, b"abc\0def"), None);

		let html = extract::parse(DocumentKind::Html, b"<h1>Title</h1><p>Body paragraph</p>").expect("html");
		assert!(html.contains("Title") && html.contains("Body paragraph"));

		let pdf_bytes = sample_pdf(&["Hello page one", "Hello page two"]);
		let pdf_text = extract::parse(DocumentKind::Pdf, &pdf_bytes).expect("pdf");
		assert!(pdf_text.contains("<!-- page 1 -->") && pdf_text.contains("Hello page one"), "{pdf_text}");
		assert!(pdf_text.contains("<!-- page 2 -->") && pdf_text.contains("Hello page two"), "{pdf_text}");
		assert_eq!(extract::page_count(&pdf_text), Some(2));
	}

	#[test]
	fn embedding_helpers_normalize_and_split_batches() {
		let mut unit = vec![3.0f32, 4.0];
		assert!(normalize_vector(&mut unit));
		assert!((unit[0] - 0.6).abs() < 1e-6 && (unit[1] - 0.8).abs() < 1e-6);
		let mut empty: Vec<f32> = Vec::new();
		assert!(!normalize_vector(&mut empty));
		let mut zeros = vec![0.0f32, 0.0];
		assert!(!normalize_vector(&mut zeros));
		let mut nan = vec![f32::NAN, 1.0];
		assert!(!normalize_vector(&mut nan));

		let many: Vec<String> = (0..130).map(|index| format!("item {index}")).collect();
		let batches = split_embedding_batches(&many);
		assert_eq!(batches.len(), 3);
		assert_eq!(batches[0].len(), 64);
		assert_eq!(batches[1].len(), 64);
		assert_eq!(batches[2].len(), 2);

		let big: Vec<String> = (0..4).map(|_| "word ".repeat(6_000)).collect();
		assert!(split_embedding_batches(&big).len() >= 2, "token cap should split oversized inputs");
	}
}

#[cfg(test)]
mod managed {
	use super::validation::request;
	use crate::tests::gateway::support::job_state;
	use crate::tests::support::{TestResult, create_user, insert_user};
	use crate::types::consts::{ADMIN_RAG_EDIT, SETTINGS_KNOWLEDGE_EDIT};
	use crate::types::rag::{
		ChunkingSettings, ConnectionKind, ConnectionMode, DocumentStatus, EmbeddingSettings, KnowledgeActor, KnowledgeBase, KnowledgeScope, ModelTargets, RagConfig,
		RagConnection, RagDefaults, RagDocument, RagError, RerankerSettings, SaveKnowledgeBaseRequest, UpdateRagSetupRequest, UploadQuotaSettings, test_fts_chunk_ids,
		test_verified_chunk_ids, user_identity_labels,
	};
	use crate::types::{AssignmentKind, AssignmentRef, AssignmentRole, ReplaceAssignmentsRequest, User};
	use sqlx::PgPool;
	use uuid::Uuid;

	async fn embed_model(pool: &PgPool, kind: &str, name: &str) -> TestResult<(Uuid, Uuid)> {
		let provider: Uuid = sqlx::query_scalar("INSERT INTO providers (name, kind, base_url) VALUES ($1, $2::provider_kind, 'https://example.com') RETURNING id")
			.bind(format!("p-{name}-{}", Uuid::new_v4()))
			.bind(kind)
			.fetch_one(pool)
			.await?;
		let model: Uuid = sqlx::query_scalar(
			"INSERT INTO models (provider_id, model_id, display_name, output_modalities) VALUES ($1, $2, $2, '[\"EMBEDDINGS\"]'::jsonb) RETURNING id",
		)
		.bind(provider)
		.bind(name)
		.fetch_one(pool)
		.await?;
		Ok((provider, model))
	}

	async fn team_with_members(pool: &PgPool, admin: &Uuid, member: &Uuid) -> TestResult<Uuid> {
		let team: Uuid = sqlx::query_scalar("INSERT INTO teams (name) VALUES ($1) RETURNING id")
			.bind(format!("team-{}", Uuid::new_v4()))
			.fetch_one(pool)
			.await?;
		sqlx::query("INSERT INTO team_members (team_id, user_id, role) VALUES ($1, $2, 'admin'), ($1, $3, 'member')")
			.bind(team)
			.bind(admin)
			.bind(member)
			.execute(pool)
			.await?;
		Ok(team)
	}

	async fn setup_managed_connection(pool: &PgPool, admin: &Uuid) -> TestResult<(RagConnection, Uuid, Uuid)> {
		crate::utils::encryption::init_for_test([0x42; 32])?;
		let (provider, model) = embed_model(pool, "OPENAI", "text-embed").await?;
		let mut req = request(ConnectionKind::Pgvector, ConnectionMode::Managed, None, None);
		req.embedding = Some(EmbeddingSettings {
			model_id: model,
			dimensions: None,
		});
		let connection = RagConnection::create(pool, admin, &req, Some(3)).await?;
		assert_eq!(RagConfig::get(pool).await?.default_connection_id, Some(connection.id), "the first managed connection becomes the default storage");
		let update = UpdateRagSetupRequest {
			reranker: RerankerSettings { model_id: None, top_n: 6 },
			defaults: RagDefaults {
				connection_id: Some(connection.id),
			},
			chunking: Some(ChunkingSettings::default()),
			quotas: Some(UploadQuotaSettings::default()),
		};
		RagConfig::save(pool, admin, &update).await?;
		Ok((connection, provider, model))
	}

	#[sqlx::test]
	async fn scopes_creation_and_sharing_follow_role_boundaries(pool: PgPool) -> TestResult {
		let admin_id = create_user(&pool).await?;
		let team_admin_id = insert_user(&pool, "teamadmin").await?;
		let member_id = insert_user(&pool, "member").await?;
		let team_id = team_with_members(&pool, &team_admin_id, &member_id).await?;

		let admin = KnowledgeActor::for_user(&pool, admin_id, &[ADMIN_RAG_EDIT.to_string()]).await?;
		let team_admin = KnowledgeActor::for_user(&pool, team_admin_id, &[SETTINGS_KNOWLEDGE_EDIT.to_string()]).await?;
		let plain_member = KnowledgeActor::for_user(&pool, member_id, &[SETTINGS_KNOWLEDGE_EDIT.to_string()]).await?;
		let no_perm = KnowledgeActor::for_user(&pool, member_id, &[]).await?;

		assert!(
			!KnowledgeBase::scopes(&pool, &admin).await?.managed_available,
			"managed should stay unavailable until a default connection has an embedding model"
		);
		let mut personal_req = SaveKnowledgeBaseRequest {
			name: "Notes".to_string(),
			scope: Some(KnowledgeScope::Personal),
			..Default::default()
		};
		assert!(matches!(KnowledgeBase::create_managed(&pool, &plain_member, &personal_req).await, Err(RagError::NoDefaultConnection)));

		let (_conn, _, _) = setup_managed_connection(&pool, &admin_id).await?;
		assert!(KnowledgeBase::scopes(&pool, &plain_member).await?.managed_available);
		assert!(KnowledgeBase::scopes(&pool, &plain_member).await?.personal);
		assert!(KnowledgeBase::scopes(&pool, &plain_member).await?.teams.is_empty());
		assert_eq!(KnowledgeBase::scopes(&pool, &team_admin).await?.teams.len(), 1);
		assert!(!KnowledgeBase::scopes(&pool, &no_perm).await?.personal);

		assert!(matches!(KnowledgeBase::create_managed(&pool, &no_perm, &personal_req).await, Err(RagError::Forbidden)));
		personal_req.search_language = Some("klingon".to_string());
		assert!(matches!(KnowledgeBase::create_managed(&pool, &plain_member, &personal_req).await, Err(RagError::InvalidLanguage)));
		personal_req.search_language = Some("german".to_string());
		let personal_id = KnowledgeBase::create_managed(&pool, &plain_member, &personal_req).await?;
		let personal_kb = KnowledgeBase::find(&pool, &plain_member, &personal_id).await?.ok_or("missing")?;
		assert_eq!(personal_kb.search_language, "german");
		assert!(personal_kb.is_managed());

		let team_req = SaveKnowledgeBaseRequest {
			name: "Engineering".to_string(),
			scope: Some(KnowledgeScope::Team),
			team_id: Some(team_id),
			..Default::default()
		};
		assert!(matches!(KnowledgeBase::create_managed(&pool, &plain_member, &team_req).await, Err(RagError::Forbidden)));
		let team_kb_id = KnowledgeBase::create_managed(&pool, &team_admin, &team_req).await?;
		let team_kb = KnowledgeBase::find(&pool, &team_admin, &team_kb_id).await?.ok_or("missing")?;

		let install_req = SaveKnowledgeBaseRequest {
			name: "Company".to_string(),
			scope: Some(KnowledgeScope::Installation),
			..Default::default()
		};
		assert!(matches!(KnowledgeBase::create_managed(&pool, &team_admin, &install_req).await, Err(RagError::Forbidden)));
		let install_id = KnowledgeBase::create_managed(&pool, &admin, &install_req).await?;
		let install_kb = KnowledgeBase::find(&pool, &admin, &install_id).await?.ok_or("missing")?;

		let everyone = ReplaceAssignmentsRequest {
			targets: vec![AssignmentRef {
				kind: AssignmentKind::Everyone,
				id: None,
				role: None,
			}],
		};
		assert!(matches!(team_kb.replace_assignments(&pool, &team_admin, &everyone).await, Err(RagError::InvalidAssignment)));
		let member_sees_team = KnowledgeBase::find(&pool, &plain_member, &team_kb_id).await?.ok_or("member sees team kb")?;
		assert!(member_sees_team.usable && !member_sees_team.editor);

		let share_team = ReplaceAssignmentsRequest {
			targets: vec![AssignmentRef {
				kind: AssignmentKind::Team,
				id: Some(team_id),
				role: Some(AssignmentRole::Editor),
			}],
		};
		install_kb.replace_assignments(&pool, &admin, &share_team).await?;
		let as_member = KnowledgeBase::find(&pool, &plain_member, &install_id).await?.ok_or("member sees shared install kb")?;
		assert!(as_member.editor);
		Ok(())
	}

	#[sqlx::test]
	async fn document_upload_checks_quotas_rate_limits_and_model_access(pool: PgPool) -> TestResult {
		let admin_id = create_user(&pool).await?;
		let admin = KnowledgeActor::for_user(&pool, admin_id, &[ADMIN_RAG_EDIT.to_string()]).await?;
		let (conn, _, _) = setup_managed_connection(&pool, &admin_id).await?;
		let state = job_state(pool.clone());

		let kb_id = KnowledgeBase::create_managed(
			&pool,
			&admin,
			&SaveKnowledgeBaseRequest {
				name: "Docs".to_string(),
				scope: Some(KnowledgeScope::Installation),
				..Default::default()
			},
		)
		.await?;
		let kb = KnowledgeBase::find(&pool, &admin, &kb_id).await?.ok_or("missing")?;

		assert!(matches!(RagDocument::upload(&state, &admin, &kb, "bad.exe", b"data").await, Err(RagError::UnsupportedFile)));
		assert!(matches!(RagDocument::upload(&state, &admin, &kb, "empty.md", b"").await, Err(RagError::EmptyFile)));
		assert!(matches!(RagDocument::upload(&state, &admin, &kb, "  ", b"hi").await, Err(RagError::InvalidFilename)));

		let uploaded = RagDocument::upload(&state, &admin, &kb, "guide.md", b"# Hello\nWorld").await?;
		assert_eq!(uploaded.status, DocumentStatus::Pending);
		assert_eq!(uploaded.filename, "guide.md");

		RagConfig::save(
			&pool,
			&admin_id,
			&UpdateRagSetupRequest {
				reranker: RerankerSettings { model_id: None, top_n: 6 },
				defaults: RagDefaults { connection_id: Some(conn.id) },
				chunking: Some(ChunkingSettings::default()),
				quotas: Some(UploadQuotaSettings {
					max_uploads_per_hour: 1,
					storage_quota_mb: Some(100),
				}),
			},
		)
		.await?;
		assert!(matches!(
			RagDocument::upload(&state, &admin, &kb, "second.md", b"hello").await,
			Err(RagError::RateLimited)
		));

		RagConfig::save(
			&pool,
			&admin_id,
			&UpdateRagSetupRequest {
				reranker: RerankerSettings { model_id: None, top_n: 6 },
				defaults: RagDefaults { connection_id: Some(conn.id) },
				chunking: Some(ChunkingSettings::default()),
				quotas: Some(UploadQuotaSettings {
					max_uploads_per_hour: 100,
					storage_quota_mb: Some(1),
				}),
			},
		)
		.await?;
		let oversized = vec![b'a'; 1_100_000];
		assert!(matches!(
			RagDocument::upload(&state, &admin, &kb, "big.txt", &oversized).await,
			Err(RagError::StorageQuotaExceeded)
		));

		let (other_provider, _) = embed_model(&pool, "ANTHROPIC", "other").await?;
		sqlx::query("INSERT INTO knowledge_base_model_targets (knowledge_base_id, provider_id) VALUES ($1, $2)")
			.bind(kb.id)
			.bind(other_provider)
			.execute(&pool)
			.await?;
		assert!(matches!(
			RagDocument::upload(&state, &admin, &kb, "blocked.md", b"hello").await,
			Err(RagError::EmbeddingModelNotAllowed)
		));
		Ok(())
	}

	#[sqlx::test]
	async fn document_limit_access_and_fts_verification_fail_closed(pool: PgPool) -> TestResult {
		let admin_id = create_user(&pool).await?;
		let member_id = insert_user(&pool, "member").await?;
		let outsider_id = insert_user(&pool, "outsider").await?;
		let team_id = team_with_members(&pool, &admin_id, &member_id).await?;
		let admin = KnowledgeActor::for_user(&pool, admin_id, &[ADMIN_RAG_EDIT.to_string()]).await?;
		let member = KnowledgeActor::for_user(&pool, member_id, &[]).await?;
		let (conn, _, _) = setup_managed_connection(&pool, &admin_id).await?;
		let state = job_state(pool.clone());

		let kb_id = KnowledgeBase::create_managed(
			&pool,
			&admin,
			&SaveKnowledgeBaseRequest {
				name: "Policy".to_string(),
				scope: Some(KnowledgeScope::Installation),
				search_language: Some("english".to_string()),
				..Default::default()
			},
		)
		.await?;
		let kb = KnowledgeBase::find(&pool, &admin, &kb_id).await?.ok_or("missing")?;

		let doc_open = RagDocument::upload(&state, &admin, &kb, "open.md", b"vacation policy").await?;
		let doc_team = RagDocument::upload(&state, &admin, &kb, "team.md", b"vacation secret").await?;
		let doc_empty = RagDocument::upload(&state, &admin, &kb, "locked.md", b"vacation hidden").await?;
		let doc_pending = RagDocument::upload(&state, &admin, &kb, "pending.md", b"vacation draft").await?;

		let mut chunk_ids = Vec::new();
		for (doc, status, access, content) in [
			(&doc_open, "ready", None, "vacation open handbook"),
			(&doc_team, "ready", Some(vec![format!("team:{team_id}")]), "vacation team handbook"),
			(&doc_empty, "ready", Some(Vec::new()), "vacation locked handbook"),
			(&doc_pending, "processing", None, "vacation draft handbook"),
		] {
			sqlx::query("UPDATE rag_documents SET status = $2, access = $3, index_generation = 1 WHERE id = $1")
				.bind(doc.id)
				.bind(status)
				.bind(&access)
				.execute(&pool)
				.await?;
			let chunk_id: Uuid = sqlx::query_scalar(
				"INSERT INTO rag_chunks (document_id, knowledge_base_id, ordinal, content, token_count, content_hash, search, access)
				 VALUES ($1, $2, 0, $3, 4, 'hash', to_tsvector('english', $3), $4) RETURNING id",
			)
			.bind(doc.id)
			.bind(kb.id)
			.bind(content)
			.bind(&access)
			.fetch_one(&pool)
			.await?;
			chunk_ids.push(chunk_id);
		}

		let member_labels = user_identity_labels(&pool, &member_id).await?;
		let outsider_labels = user_identity_labels(&pool, &outsider_id).await?;

		assert_eq!(test_fts_chunk_ids(&pool, &kb, &conn, "vacation", &member_labels).await.len(), 2);
		assert_eq!(test_fts_chunk_ids(&pool, &kb, &conn, "vacation", &outsider_labels).await, [chunk_ids[0]]);
		assert_eq!(
			test_verified_chunk_ids(&pool, &kb.id, &chunk_ids, &member_labels).await?,
			vec![chunk_ids[0], chunk_ids[1]]
		);
		assert_eq!(
			test_verified_chunk_ids(&pool, &kb.id, &chunk_ids, &outsider_labels).await?,
			vec![chunk_ids[0]]
		);

		assert!(matches!(
			RagDocument::replace_assignments(
				&state,
				&member,
				&kb,
				&doc_open.id,
				&ReplaceAssignmentsRequest {
					targets: vec![AssignmentRef {
						kind: AssignmentKind::Team,
						id: Some(team_id),
						role: None,
					}],
				},
			)
			.await,
			Err(RagError::Forbidden)
		));
		assert!(matches!(
			RagDocument::replace_assignments(
				&state,
				&admin,
				&kb,
				&doc_open.id,
				&ReplaceAssignmentsRequest {
					targets: vec![AssignmentRef {
						kind: AssignmentKind::Everyone,
						id: None,
						role: None,
					}],
				},
			)
			.await,
			Err(RagError::InvalidAssignment)
		));

		RagDocument::replace_assignments(
			&state,
			&admin,
			&kb,
			&doc_open.id,
			&ReplaceAssignmentsRequest {
				targets: vec![AssignmentRef {
					kind: AssignmentKind::Team,
					id: Some(team_id),
					role: None,
				}],
			},
		)
		.await?;
		assert!(test_verified_chunk_ids(&pool, &kb.id, &[chunk_ids[0]], &outsider_labels).await?.is_empty());
		assert_eq!(test_verified_chunk_ids(&pool, &kb.id, &[chunk_ids[0]], &member_labels).await?, [chunk_ids[0]]);
		assert_eq!(RagDocument::assignments(&pool, &admin, &kb, &doc_open.id).await?.targets.len(), 1);

		RagDocument::replace_assignments(&state, &admin, &kb, &doc_open.id, &ReplaceAssignmentsRequest { targets: Vec::new() }).await?;
		assert_eq!(test_verified_chunk_ids(&pool, &kb.id, &[chunk_ids[0]], &outsider_labels).await?, [chunk_ids[0]]);
		Ok(())
	}

	#[sqlx::test]
	async fn deletion_user_delete_and_connection_reindex_behave_safely(pool: PgPool) -> TestResult {
		let admin_id = create_user(&pool).await?;
		let alice_id = insert_user(&pool, "alice").await?;
		let admin = KnowledgeActor::for_user(&pool, admin_id, &[ADMIN_RAG_EDIT.to_string()]).await?;
		let alice = KnowledgeActor::for_user(&pool, alice_id, &[SETTINGS_KNOWLEDGE_EDIT.to_string()]).await?;
		let (conn, provider_a, _) = setup_managed_connection(&pool, &admin_id).await?;
		let (_, model_b) = embed_model(&pool, "ANTHROPIC", "embed-b").await?;
		let state = job_state(pool.clone());

		let kb_id = KnowledgeBase::create_managed(
			&pool,
			&admin,
			&SaveKnowledgeBaseRequest {
				name: "Restricted".to_string(),
				scope: Some(KnowledgeScope::Installation),
				..Default::default()
			},
		)
		.await?;
		let kb = KnowledgeBase::find(&pool, &admin, &kb_id).await?.ok_or("missing")?;
		let doc = RagDocument::upload(&state, &admin, &kb, "a.md", b"hello world").await?;
		sqlx::query("UPDATE rag_documents SET status = 'ready', chunk_count = 3, token_count = 120, index_generation = $2 WHERE id = $1")
			.bind(doc.id)
			.bind(conn.index_generation)
			.execute(&pool)
			.await?;
		sqlx::query(
			"INSERT INTO rag_chunks (document_id, knowledge_base_id, ordinal, content, token_count, content_hash, search)
			 VALUES ($1, $2, 0, 'one', 40, 'h1', to_tsvector('simple', 'one')),
			        ($1, $2, 1, 'two', 40, 'h2', to_tsvector('simple', 'two')),
			        ($1, $2, 2, 'three', 40, 'h3', to_tsvector('simple', 'three'))",
		)
		.bind(doc.id)
		.bind(kb.id)
		.execute(&pool)
		.await?;
		kb.replace_model_access(
			&pool,
			&admin,
			&ModelTargets {
				providers: vec![provider_a],
				..Default::default()
			},
		)
		.await?;

		let embed_b = EmbeddingSettings {
			model_id: model_b,
			dimensions: None,
		};
		let estimate = conn.reindex_estimate(&pool, &embed_b).await?;
		assert_eq!(estimate.document_count, 1);
		assert_eq!(estimate.chunk_count, 3);
		assert_eq!(estimate.conflicting_knowledge_bases, ["Restricted"]);

		let mut switch_req = request(ConnectionKind::Pgvector, ConnectionMode::Managed, None, None);
		switch_req.embedding = Some(embed_b);
		assert!(matches!(
			conn.update(&pool, &admin_id, &switch_req, Some(3)).await,
			Err(RagError::ConnectionModelConflict(names)) if names == ["Restricted"]
		));

		kb.replace_model_access(&pool, &admin, &ModelTargets::default()).await?;
		let updated_conn = conn.update(&pool, &admin_id, &switch_req, Some(3)).await?;
		assert_eq!(updated_conn.index_generation, conn.index_generation + 1);
		assert!(updated_conn.is_reindexing(&pool).await?);
		let doc_after = RagDocument::find(&pool, &kb.id, &doc.id).await?.ok_or("doc")?;
		assert_eq!(doc_after.status, DocumentStatus::Pending);

		kb.delete(&pool, &admin).await?;
		assert!(KnowledgeBase::find(&pool, &admin, &kb.id).await?.is_none(), "soft-deleted KB is hidden");
		let doc_deleting = RagDocument::find(&pool, &kb.id, &doc.id).await?.ok_or("doc stays until worker deletes vectors")?;
		assert_eq!(doc_deleting.status, DocumentStatus::Deleting);

		let alice_kb_id = KnowledgeBase::create_managed(
			&pool,
			&alice,
			&SaveKnowledgeBaseRequest {
				name: "Alice Notes".to_string(),
				scope: Some(KnowledgeScope::Personal),
				..Default::default()
			},
		)
		.await?;
		let alice_kb = KnowledgeBase::find(&pool, &alice, &alice_kb_id).await?.ok_or("alice kb")?;
		let alice_doc = RagDocument::upload(&state, &alice, &alice_kb, "note.md", b"personal").await?;
		let alice_user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
			.bind(alice_id)
			.fetch_one(&pool)
			.await?;
		alice_user.delete(&pool).await?;
		let alice_doc_after = RagDocument::find(&pool, &alice_kb.id, &alice_doc.id).await?.ok_or("doc waits for worker")?;
		assert_eq!(alice_doc_after.status, DocumentStatus::Deleting);
		let kb_soft_deleted: bool = sqlx::query_scalar("SELECT deleted_at IS NOT NULL FROM knowledge_bases WHERE id = $1")
			.bind(alice_kb.id)
			.fetch_one(&pool)
			.await?;
		assert!(kb_soft_deleted, "personal KB with docs must soft-delete so worker can purge vectors");
		Ok(())
	}

	#[sqlx::test]
	async fn failed_documents_restricted_listings_team_delete_and_upload_rate(pool: PgPool) -> TestResult {
		let admin_id = create_user(&pool).await?;
		let member_id = insert_user(&pool, "member").await?;
		let team_id = team_with_members(&pool, &admin_id, &member_id).await?;
		let admin = KnowledgeActor::for_user(&pool, admin_id, &[ADMIN_RAG_EDIT.to_string()]).await?;
		let member = KnowledgeActor::for_user(&pool, member_id, &[]).await?;
		let (conn, _, _) = setup_managed_connection(&pool, &admin_id).await?;
		let state = job_state(pool.clone());

		let kb_id = KnowledgeBase::create_managed(
			&pool,
			&admin,
			&SaveKnowledgeBaseRequest {
				name: "Team docs".to_string(),
				scope: Some(KnowledgeScope::Team),
				team_id: Some(team_id),
				..Default::default()
			},
		)
		.await?;
		let kb = KnowledgeBase::find(&pool, &admin, &kb_id).await?.ok_or("missing")?;
		let open = RagDocument::upload(&state, &admin, &kb, "open.md", b"open").await?;
		let secret = RagDocument::upload(&state, &admin, &kb, "secret.md", b"secret").await?;
		sqlx::query("UPDATE rag_documents SET access = $2 WHERE id = $1")
			.bind(secret.id)
			.bind(vec![format!("user:{admin_id}")])
			.execute(&pool)
			.await?;

		let as_member = KnowledgeBase::find(&pool, &member, &kb_id).await?.ok_or("member sees team kb")?;
		let seen: Vec<String> = RagDocument::visible_to(&pool, &member, &as_member).await?.into_iter().map(|doc| doc.filename).collect();
		assert_eq!(seen, ["open.md"], "a viewer must not see documents limited to others");
		assert!(matches!(RagDocument::assignments(&pool, &member, &as_member, &secret.id).await, Err(RagError::Forbidden)));
		assert_eq!(RagDocument::visible_to(&pool, &admin, &kb).await?.len(), 2);

		sqlx::query("UPDATE rag_documents SET status = 'failed', index_generation = $2 WHERE id = $1")
			.bind(open.id)
			.bind(conn.index_generation - 1)
			.execute(&pool)
			.await?;
		sqlx::query("UPDATE rag_documents SET status = 'ready' WHERE id = $1").bind(secret.id).execute(&pool).await?;
		assert!(!conn.is_reindexing(&pool).await?, "a failed document must not block search");
		assert!(RagConnection::reindex_progress_map(&pool).await?.is_empty());

		let quotas = UpdateRagSetupRequest {
			reranker: RerankerSettings { model_id: None, top_n: 6 },
			defaults: RagDefaults { connection_id: Some(conn.id) },
			chunking: Some(ChunkingSettings::default()),
			quotas: Some(UploadQuotaSettings {
				max_uploads_per_hour: 2,
				storage_quota_mb: Some(100),
			}),
		};
		RagConfig::save(&pool, &admin_id, &quotas).await?;
		sqlx::query("DELETE FROM rag_documents WHERE id = $1").bind(open.id).execute(&pool).await?;
		assert!(
			matches!(RagDocument::upload(&state, &admin, &kb, "again.md", b"again").await, Err(RagError::RateLimited)),
			"a deleted upload still counts towards the hourly limit"
		);

		let team = crate::types::Team::find_by_id(&pool, &team_id).await?.ok_or("team")?;
		assert!(team.delete(&pool).await?, "a team that owns a knowledge base can be deleted");
		assert!(KnowledgeBase::find(&pool, &admin, &kb_id).await?.is_none());
		let status: DocumentStatus = sqlx::query_scalar("SELECT status FROM rag_documents WHERE id = $1").bind(secret.id).fetch_one(&pool).await?;
		assert_eq!(status, DocumentStatus::Deleting, "the worker still has vectors to purge");
		Ok(())
	}
}
