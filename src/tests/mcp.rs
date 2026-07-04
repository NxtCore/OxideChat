#[cfg(test)]
mod tests {
	use crate::types::tools::{McpServer, Tool, ToolSourceKind};
	use sqlx::PgPool;
	use uuid::Uuid;

	async fn create_user(pool: &PgPool, suffix: &str) -> Uuid {
		sqlx::query_scalar::<_, Uuid>(
			r#"
			INSERT INTO users (email, username, password_hash)
			VALUES ($1, $2, 'hash')
			RETURNING id
			"#,
		)
		.bind(format!("{suffix}@example.com"))
		.bind(format!("user_{suffix}"))
		.fetch_one(pool)
		.await
		.unwrap()
	}

	fn http_config(url: &str) -> serde_json::Value {
		serde_json::json!({ "url": url, "headers": {} })
	}

	async fn insert_tool(pool: &PgPool, owner_id: Option<&Uuid>, name: &str) -> Uuid {
		sqlx::query_scalar::<_, Uuid>(
			r#"
			INSERT INTO tools (owner_id, name, display_name, source_kind, source_config, input_schema, is_enabled)
			VALUES ($1, $2, $2, 'HTTP', '{}'::jsonb, '{"type":"object"}'::jsonb, true)
			RETURNING id
			"#,
		)
		.bind(owner_id)
		.bind(name)
		.fetch_one(pool)
		.await
		.unwrap()
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn create_and_scope_servers(pool: PgPool) {
		let user = create_user(&pool, "mcp_owner").await;

		let system = McpServer::create(&pool, None, "sys", "http", &http_config("https://sys.example"), true)
			.await
			.unwrap();
		let owned = McpServer::create(&pool, Some(&user), "mine", "http", &http_config("https://mine.example"), true)
			.await
			.unwrap();

		// list_owned returns only the user's servers, not system ones.
		let owned_list = McpServer::list_owned(&pool, &user).await.unwrap();
		assert_eq!(owned_list.len(), 1);
		assert_eq!(owned_list[0].id, owned.id);

		// list_system returns only system servers.
		let system_list = McpServer::list_system(&pool).await.unwrap();
		assert_eq!(system_list.len(), 1);
		assert_eq!(system_list[0].id, system.id);

		// A user can resolve their own and system servers for execution, but not another owner's.
		assert!(McpServer::find_owned_or_system(&pool, &owned.id, &user).await.unwrap().is_some());
		assert!(McpServer::find_owned_or_system(&pool, &system.id, &user).await.unwrap().is_some());

		let other = create_user(&pool, "mcp_other").await;
		assert!(McpServer::find_owned_or_system(&pool, &owned.id, &other).await.unwrap().is_none());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn update_and_delete_are_owner_scoped(pool: PgPool) {
		let user = create_user(&pool, "mcp_scope").await;
		let other = create_user(&pool, "mcp_scope_other").await;
		let server = McpServer::create(&pool, Some(&user), "srv", "http", &http_config("https://a.example"), true)
			.await
			.unwrap();

		// A different owner scope cannot update or delete the server.
		assert!(
			McpServer::update_scoped(&pool, &server.id, Some(&other), Some("hacked"), None, None, None)
				.await
				.unwrap()
				.is_none()
		);
		assert!(!McpServer::delete_scoped(&pool, &server.id, Some(&other)).await.unwrap());
		assert!(!McpServer::delete_scoped(&pool, &server.id, None).await.unwrap());

		// The real owner can.
		let updated = McpServer::update_scoped(&pool, &server.id, Some(&user), Some("renamed"), None, None, Some(false))
			.await
			.unwrap()
			.unwrap();
		assert_eq!(updated.name, "renamed");
		assert!(!updated.is_enabled);
		assert!(McpServer::delete_scoped(&pool, &server.id, Some(&user)).await.unwrap());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn sync_tools_creates_and_replaces_records(pool: PgPool) {
		let user = create_user(&pool, "mcp_sync").await;
		let server = McpServer::create(&pool, Some(&user), "toolsrv", "http", &http_config("https://t.example"), true)
			.await
			.unwrap();

		let discovered = vec![
			crate::utils::tools::mcp::McpToolInfo {
				name: "search".to_string(),
				description: Some("Search".to_string()),
				input_schema: serde_json::json!({"type": "object"}),
			},
			crate::utils::tools::mcp::McpToolInfo {
				name: "fetch".to_string(),
				description: None,
				input_schema: serde_json::json!({"type": "object"}),
			},
		];

		let created = server.sync_tools(&pool, Some(&user), &discovered).await.unwrap();
		assert_eq!(created.len(), 2);
		assert!(created.iter().all(|t| t.source_kind == ToolSourceKind::Mcp));

		let names = Tool::names_for_mcp_server(&pool, &server.id, Some(&user)).await.unwrap();
		assert_eq!(names.len(), 2);

		// Re-syncing with a smaller set replaces (does not accumulate).
		let smaller = vec![crate::utils::tools::mcp::McpToolInfo {
			name: "search".to_string(),
			description: Some("Search".to_string()),
			input_schema: serde_json::json!({"type": "object"}),
		}];
		let created2 = server.sync_tools(&pool, Some(&user), &smaller).await.unwrap();
		assert_eq!(created2.len(), 1);
		let names2 = Tool::names_for_mcp_server(&pool, &server.id, Some(&user)).await.unwrap();
		assert_eq!(names2.len(), 1);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn lookup_prefers_owned_over_global(pool: PgPool) {
		let user = create_user(&pool, "mcp_lookup").await;

		// Same name owned by the user and globally.
		let global_id = insert_tool(&pool, None, "shared").await;
		let owned_id = insert_tool(&pool, Some(&user), "shared").await;

		let found = Tool::find_enabled_by_name_for_user(&pool, "shared", &user).await.unwrap().unwrap();
		assert_eq!(found.id, owned_id, "user-owned tool should win on name collision");

		// A global-only tool is still resolvable by name.
		insert_tool(&pool, None, "global_only").await;
		let g = Tool::find_enabled_by_name_for_user(&pool, "global_only", &user).await.unwrap().unwrap();
		assert_eq!(g.owner_id, None);

		// find_enabled_for_user includes both owned and global by id.
		let by_ids = Tool::find_enabled_for_user(&pool, &[global_id, owned_id], &user).await.unwrap();
		assert_eq!(by_ids.len(), 2);

		// Another user cannot resolve the first user's owned tool by name (falls back to global).
		let other = create_user(&pool, "mcp_lookup_other").await;
		let found_other = Tool::find_enabled_by_name_for_user(&pool, "shared", &other).await.unwrap().unwrap();
		assert_eq!(found_other.id, global_id);
	}
}
