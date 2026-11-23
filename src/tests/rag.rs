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
		];
		assert_eq!(pg_access("oxide_access", &columns, 4, 5), Some(("t.\"oxide_access\" && $4::text[]".to_string(), Vec::new())));
		assert_eq!(
			pg_access("metadata.acl.groups", &columns, 4, 5),
			Some(("(to_jsonb(t.\"metadata\") #> $5::text[]) ?| $4::text[]".to_string(), vec!["acl".to_string(), "groups".to_string()]))
		);
		assert_eq!(
			pg_access("metadata", &columns, 4, 5),
			Some(("(to_jsonb(t.\"metadata\") #> $5::text[]) ?| $4::text[]".to_string(), Vec::new()))
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
		KnowledgeActor {
			user_id,
			manage: false,
			view_all: false,
		}
	}

	async fn fixture(pool: &PgPool) -> TestResult<Fixture> {
		let admin = KnowledgeActor {
			user_id: create_user(pool).await?,
			manage: true,
			view_all: true,
		};
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
			view: None,
		};
		base.update(&pool, &editor, &rename, None).await?;
		rename.view = Some(base.view.0.clone());
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
			let admin = fixture.admin;
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
		Ok(parse_version(&version) >= (0, 8))
	}

	#[sqlx::test]
	async fn search_only_finds_records_listing_a_held_label(pool: PgPool) -> TestResult {
		if !labelled(&pool).await? {
			return Ok(());
		}
		let store = VectorStore::Pgvector(Cow::Borrowed(&pool));
		for field in ["oxide_access", "metadata.acl.groups"] {
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
}

#[cfg(test)]
mod retrieval {
	use super::validation::request;
	use super::views::view;
	use crate::tests::gateway::support::job_state;
	use crate::tests::support::{TestResult, create_user, insert_user};
	use crate::types::rag::{
		Candidate, ConnectionKind, ConnectionMode, KNOWLEDGE_TOOL, KnowledgeActor, KnowledgeBase, KnowledgeHit, RagConfig, RagConnection, RagDefaults, RerankerSettings,
		SaveKnowledgeBaseRequest, UpdateRagSetupRequest, add_knowledge_tool, number, per_document, rrf, run_knowledge_tool, turn_knowledge_bases,
	};
	use crate::types::{AssignmentKind, AssignmentRef, ReplaceAssignmentsRequest};
	use omniference::types::{ChatRequestIR, ToolSpec};
	use serde_json::json;
	use sqlx::PgPool;
	use uuid::Uuid;

	fn candidate(base: Uuid, text: &str, url: Option<&str>) -> Candidate {
		Candidate {
			knowledge_base: base,
			knowledge_base_name: "Handbook".to_string(),
			hit: KnowledgeHit {
				text: text.to_string(),
				title: None,
				url: url.map(str::to_string),
				section: None,
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
		assert_eq!(sources[0].text.chars().count(), 4_001);
		let many: Vec<Candidate> = (0..10).map(|_| candidate(base, &long, None)).collect();
		assert_eq!(number(many).len(), 3);
	}

	async fn shared_base(pool: &PgPool, name: &str) -> TestResult<(KnowledgeBase, KnowledgeActor)> {
		crate::utils::encryption::init_for_test([0x42; 32])?;
		let admin = KnowledgeActor {
			user_id: create_user(pool).await?,
			manage: true,
			view_all: true,
		};
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
		};
		let id = KnowledgeBase::create(pool, &admin, &save, &external).await?;
		Ok((KnowledgeBase::find(pool, &admin, &id).await?.ok_or("missing")?, admin))
	}

	async fn enable(pool: &PgPool, actor: &Uuid, enabled: bool) -> TestResult {
		let setup = UpdateRagSetupRequest {
			enabled,
			reranker: RerankerSettings { model_id: None, top_n: 6 },
			defaults: RagDefaults { connection_id: None },
		};
		Ok(RagConfig::save(pool, actor, &setup).await?)
	}

	#[sqlx::test]
	async fn only_shared_bases_of_the_turn_are_searched_while_knowledge_is_on(pool: PgPool) -> TestResult {
		let (base, admin) = shared_base(&pool, "Handbook").await?;
		let member = insert_user(&pool, "member").await?;
		let state = job_state(pool.clone());

		enable(&pool, &admin.user_id, true).await?;
		assert!(turn_knowledge_bases(&state, &member, &[base.id]).await?.is_empty());
		assert!(turn_knowledge_bases(&state, &member, &[]).await?.is_empty());
		let unshared = run_knowledge_tool(&state, &member, &[base.id], &json!({"query": "holidays"})).await?;
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
		let broken = run_knowledge_tool(&state, &member, &[base.id], &json!({"query": "holidays"})).await?;
		assert!(broken.sources.is_empty());
		assert_eq!(broken.unavailable, ["Handbook"]);
		let other = run_knowledge_tool(&state, &member, &[base.id], &json!({"query": "holidays", "knowledge_base": "Elsewhere"})).await?;
		assert!(other.sources.is_empty());
		assert!(run_knowledge_tool(&state, &member, &[base.id], &json!({"query": " "})).await.is_err());

		enable(&pool, &admin.user_id, false).await?;
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
	use crate::types::providers::ProviderKind;
	use crate::types::rag::{
		ConnectionKind, ConnectionMode, KNOWLEDGE_CHAT_METADATA_KEY, KnowledgeActor, KnowledgeBase, ModelAccess, ModelTargets, RagConnection, RagError, RoutedModel,
		SaveKnowledgeBaseRequest, chat_allows_model,
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
			models: Vec::new(),
		};
		assert_eq!(targets.normalized()?, ModelTargets { providers: vec![id], upstream_providers: slugs(&["azure"]), models: Vec::new() });
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
		let admin = KnowledgeActor {
			user_id: create_user(pool).await?,
			manage: true,
			view_all: true,
		};
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
			description: None,
			view: Some(external.clone()),
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

		let viewer = KnowledgeActor { user_id: create_user(&pool).await?, manage: false, view_all: false };
		let targets = ModelTargets { providers: vec![embedding_provider], models: vec![chat_model], ..Default::default() };
		assert!(matches!(base.replace_model_access(&pool, &viewer, &targets).await, Err(RagError::Forbidden)));

		base.replace_model_access(&pool, &admin, &targets).await?;
		let stored = base.model_access(&pool, &admin).await?.targets;
		assert_eq!(stored.providers, [embedding_provider]);
		assert_eq!(stored.models, [chat_model]);
		let audited: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE event = 'knowledge_base_model_access_updated' AND target_id = $1")
			.bind(base.id)
			.fetch_one(&pool)
			.await?;
		assert_eq!(audited, 1);
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
			models: Vec::new(),
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
		KnowledgeBase::copy_chat_sources(&pool, &chat_id, &branch).await?;
		assert!(!chat_allows_model(&pool, &branch, &other).await?, "a branch keeps the restriction");

		base.delete(&pool, &admin).await?;
		assert!(!chat_allows_model(&pool, &chat_id, &other).await?, "deleting the knowledge base keeps the restriction");
		Ok(())
	}
}
