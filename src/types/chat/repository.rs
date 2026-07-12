use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::rows::{SiblingCountRow, ToolExecutionRow, WorkspaceWithCount};
use super::{
	Chat, CostDetails, Message, ReasoningDetails, RequestSettings, StreamingAssistantMessageCreate, StreamingUserMessageCreate, ToolExecutionResponse,
	UpdatePreferencesRequest, UsageDetails, UserPreferences, Workspace, WorkspaceResponse,
};

impl Workspace {
	pub async fn list_with_counts(pool: &PgPool, user_id: &Uuid) -> Result<Vec<WorkspaceResponse>, sqlx::Error> {
		let rows = sqlx::query_as::<_, WorkspaceWithCount>(
			r#"
			SELECT w.*, COALESCE(c.chat_count, 0) AS chat_count
			FROM workspaces w
			LEFT JOIN (
				SELECT workspace_id, COUNT(*) AS chat_count
				FROM chats
				WHERE is_archived = false
				GROUP BY workspace_id
			) c ON w.id = c.workspace_id
			WHERE w.user_id = $1
			ORDER BY w.sort_order, w.name
			"#,
		)
		.bind(user_id)
		.fetch_all(pool)
		.await?;

		Ok(rows.into_iter().map(WorkspaceResponse::from).collect())
	}

	pub async fn find_with_count(pool: &PgPool, id: &Uuid, user_id: &Uuid) -> Result<Option<WorkspaceResponse>, sqlx::Error> {
		let row = sqlx::query_as::<_, WorkspaceWithCount>(
			r#"
			SELECT w.*, COALESCE(c.chat_count, 0) AS chat_count
			FROM workspaces w
			LEFT JOIN (
				SELECT workspace_id, COUNT(*) AS chat_count
				FROM chats
				WHERE is_archived = false
				GROUP BY workspace_id
			) c ON w.id = c.workspace_id
			WHERE w.id = $1 AND w.user_id = $2
			"#,
		)
		.bind(id)
		.bind(user_id)
		.fetch_optional(pool)
		.await?;

		Ok(row.map(WorkspaceResponse::from))
	}

	pub async fn find_by_id_and_user(pool: &PgPool, id: &Uuid, user_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Workspace>("SELECT * FROM workspaces WHERE id = $1 AND user_id = $2")
			.bind(id)
			.bind(user_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn create(pool: &PgPool, user_id: &Uuid, name: &str, icon: Option<&str>, color: Option<&str>, is_default: bool) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Workspace>(
			r#"
			INSERT INTO workspaces (user_id, name, icon, color, is_default)
			VALUES ($1, $2, $3, $4, $5)
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(name)
		.bind(icon)
		.bind(color)
		.bind(is_default)
		.fetch_one(pool)
		.await
	}
}

impl Chat {
	pub async fn list_by_user(
		pool: &PgPool,
		user_id: &Uuid,
		workspace_id: Option<&Uuid>,
		include_archived: bool,
		limit: i64,
		offset: i64,
	) -> Result<Vec<Self>, sqlx::Error> {
		if include_archived {
			if let Some(ws_id) = workspace_id {
				sqlx::query_as::<_, Chat>(
					r#"
					SELECT * FROM chats
					WHERE user_id = $1 AND workspace_id = $2
					ORDER BY is_pinned DESC, updated_at DESC
					LIMIT $3 OFFSET $4
					"#,
				)
				.bind(user_id)
				.bind(ws_id)
				.bind(limit)
				.bind(offset)
				.fetch_all(pool)
				.await
			} else {
				sqlx::query_as::<_, Chat>(
					r#"
					SELECT * FROM chats
					WHERE user_id = $1
					ORDER BY is_pinned DESC, updated_at DESC
					LIMIT $2 OFFSET $3
					"#,
				)
				.bind(user_id)
				.bind(limit)
				.bind(offset)
				.fetch_all(pool)
				.await
			}
		} else if let Some(ws_id) = workspace_id {
			sqlx::query_as::<_, Chat>(
				r#"
				SELECT * FROM chats
				WHERE user_id = $1 AND workspace_id = $2 AND is_archived = false
				ORDER BY is_pinned DESC, updated_at DESC
				LIMIT $3 OFFSET $4
				"#,
			)
			.bind(user_id)
			.bind(ws_id)
			.bind(limit)
			.bind(offset)
			.fetch_all(pool)
			.await
		} else {
			sqlx::query_as::<_, Chat>(
				r#"
				SELECT * FROM chats
				WHERE user_id = $1 AND is_archived = false
				ORDER BY is_pinned DESC, updated_at DESC
				LIMIT $2 OFFSET $3
				"#,
			)
			.bind(user_id)
			.bind(limit)
			.bind(offset)
			.fetch_all(pool)
			.await
		}
	}

	pub async fn find_by_id_and_user(pool: &PgPool, id: &Uuid, user_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Chat>("SELECT * FROM chats WHERE id = $1 AND user_id = $2")
			.bind(id)
			.bind(user_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn exists_for_user(pool: &PgPool, id: &Uuid, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		let exists: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM chats WHERE id = $1 AND user_id = $2)")
			.bind(id)
			.bind(user_id)
			.fetch_one(pool)
			.await?;
		Ok(exists.0)
	}

	pub async fn create(pool: &PgPool, user_id: &Uuid, workspace_id: Option<&Uuid>, title: Option<&str>) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Chat>(
			r#"
			INSERT INTO chats (user_id, workspace_id, title)
			VALUES ($1, $2, $3)
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(workspace_id)
		.bind(title)
		.fetch_one(pool)
		.await
	}

	pub async fn create_branched(
		pool: &PgPool,
		user_id: &Uuid,
		workspace_id: Option<&Uuid>,
		title: Option<&str>,
		branched_from_chat_id: &Uuid,
		branched_from_message_id: &Uuid,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Chat>(
			r#"
			INSERT INTO chats (user_id, workspace_id, title, branched_from_chat_id, branched_from_message_id)
			VALUES ($1, $2, $3, $4, $5)
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(workspace_id)
		.bind(title)
		.bind(branched_from_chat_id)
		.bind(branched_from_message_id)
		.fetch_one(pool)
		.await
	}

	pub async fn message_stats(pool: &PgPool, chat_id: &Uuid) -> Result<(i64, Option<DateTime<Utc>>), sqlx::Error> {
		let row: (i64, Option<DateTime<Utc>>) = sqlx::query_as("SELECT COUNT(*), MAX(created_at) FROM messages WHERE chat_id = $1")
			.bind(chat_id)
			.fetch_one(pool)
			.await?;
		Ok(row)
	}

	pub async fn message_stats_batch(pool: &PgPool, chat_ids: &[Uuid]) -> Result<HashMap<Uuid, (i64, Option<DateTime<Utc>>)>, sqlx::Error> {
		if chat_ids.is_empty() {
			return Ok(HashMap::new());
		}
		let rows: Vec<(Uuid, i64, Option<DateTime<Utc>>)> = sqlx::query_as(
			r#"
			SELECT chat_id, COUNT(*)::int8, MAX(created_at)
			FROM messages
			WHERE chat_id = ANY($1)
			GROUP BY chat_id
			"#,
		)
		.bind(chat_ids)
		.fetch_all(pool)
		.await?;
		Ok(rows.into_iter().map(|(id, count, last)| (id, (count, last))).collect())
	}

	pub async fn verify_workspace_belongs_to_user(pool: &PgPool, workspace_id: &Uuid, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		let exists: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = $1 AND user_id = $2)")
			.bind(workspace_id)
			.bind(user_id)
			.fetch_one(pool)
			.await?;
		Ok(exists.0)
	}
}

impl Message {
	pub async fn last_active_id(pool: &PgPool, chat_id: &Uuid) -> Result<Option<Uuid>, sqlx::Error> {
		sqlx::query_scalar("SELECT id FROM messages WHERE chat_id = $1 AND is_active_fork = TRUE ORDER BY created_at DESC LIMIT 1")
			.bind(chat_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn list_active_by_chat(pool: &PgPool, chat_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE chat_id = $1 AND is_active_fork = true ORDER BY created_at ASC")
			.bind(chat_id)
			.fetch_all(pool)
			.await
	}

	pub async fn list_by_chat(pool: &PgPool, chat_id: &Uuid, before: Option<&Uuid>, after: Option<&Uuid>) -> Result<Vec<Self>, sqlx::Error> {
		if let Some(before_id) = before {
			return sqlx::query_as::<_, Message>(
				r#"
				SELECT * FROM messages
				WHERE chat_id = $1 AND is_active_fork = TRUE AND created_at < (SELECT created_at FROM messages WHERE id = $2)
				ORDER BY created_at DESC
				"#,
			)
			.bind(chat_id)
			.bind(before_id)
			.fetch_all(pool)
			.await;
		}
		if let Some(after_id) = after {
			return sqlx::query_as::<_, Message>(
				r#"
				SELECT * FROM messages
				WHERE chat_id = $1 AND is_active_fork = TRUE AND created_at > (SELECT created_at FROM messages WHERE id = $2)
				ORDER BY created_at ASC
				"#,
			)
			.bind(chat_id)
			.bind(after_id)
			.fetch_all(pool)
			.await;
		}
		sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE chat_id = $1 AND is_active_fork = TRUE ORDER BY created_at ASC")
			.bind(chat_id)
			.fetch_all(pool)
			.await
	}

	pub async fn list_active_before(pool: &PgPool, chat_id: &Uuid, before_at: DateTime<Utc>) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE chat_id = $1 AND is_active_fork = TRUE AND created_at < $2 ORDER BY created_at ASC")
			.bind(chat_id)
			.bind(before_at)
			.fetch_all(pool)
			.await
	}

	pub async fn list_active_up_to(pool: &PgPool, chat_id: &Uuid, up_to_at: DateTime<Utc>) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE chat_id = $1 AND is_active_fork = TRUE AND created_at <= $2 ORDER BY created_at ASC")
			.bind(chat_id)
			.bind(up_to_at)
			.fetch_all(pool)
			.await
	}

	pub async fn find_by_id_and_chat(pool: &PgPool, id: &Uuid, chat_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = $1 AND chat_id = $2")
			.bind(id)
			.bind(chat_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn create_streaming_user(pool: &PgPool, params: StreamingUserMessageCreate<'_>) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Message>(
			r#"
			INSERT INTO messages (chat_id, role, content, content_parts, model_id, reasoning_details, usage_details, cost_details, request_settings, parent_id)
			VALUES ($1, 'user', $2, $3, $4, $5, $6, $7, $8, $9)
			RETURNING *
			"#,
		)
		.bind(params.chat_id)
		.bind(params.content)
		.bind(params.content_parts)
		.bind(params.model_id)
		.bind(sqlx::types::Json(params.reasoning_details))
		.bind(sqlx::types::Json(params.usage_details))
		.bind(sqlx::types::Json(params.cost_details))
		.bind(sqlx::types::Json(params.request_settings))
		.bind(params.parent_id)
		.fetch_one(pool)
		.await
	}

	pub async fn next_assistant_fork_index(pool: &PgPool, chat_id: &Uuid, parent_id: Option<Uuid>) -> Result<i32, sqlx::Error> {
		let next = sqlx::query_scalar::<_, Option<i32>>(
			r#"SELECT COALESCE(MAX(fork_index), 0) + 1 FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND role = 'assistant'"#,
		)
		.bind(chat_id)
		.bind(parent_id)
		.fetch_one(pool)
		.await?;
		Ok(next.unwrap_or(1))
	}

	pub async fn create_streaming_assistant(pool: &PgPool, params: StreamingAssistantMessageCreate<'_>) -> Result<Self, sqlx::Error> {
		let message = sqlx::query_as::<_, Message>(
			r#"
			INSERT INTO messages (
				chat_id, role, content, content_parts, reasoning_content,
				model_id, reasoning_details, usage_details, cost_details, request_settings, parent_id, fork_index, is_active_fork
			)
			VALUES ($1, 'assistant', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, TRUE)
			RETURNING *
			"#,
		)
		.bind(params.chat_id)
		.bind(params.content)
		.bind(params.content_parts)
		.bind(params.reasoning_content)
		.bind(params.model_id)
		.bind(sqlx::types::Json(params.reasoning_details))
		.bind(sqlx::types::Json(params.usage_details))
		.bind(sqlx::types::Json(params.cost_details))
		.bind(sqlx::types::Json(params.request_settings))
		.bind(params.parent_id)
		.bind(params.fork_index)
		.fetch_one(pool)
		.await?;

		Chat::touch(pool, params.chat_id).await?;
		Ok(message)
	}

	pub async fn create(
		conn: &mut sqlx::PgConnection,
		chat_id: &Uuid,
		role: &str,
		content: &str,
		reasoning_content: Option<&str>,
		model_id: Option<&Uuid>,
		parent_id: Option<&Uuid>,
		fork_index: i32,
		content_parts: Option<&serde_json::Value>,
		cost_details: Option<&CostDetails>,
		usage_details: Option<&UsageDetails>,
		reasoning_details: Option<&ReasoningDetails>,
		request_settings: Option<&RequestSettings>,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Message>(
			r#"
			INSERT INTO messages (chat_id, role, content, reasoning_content, model_id,
			                     parent_id, fork_index, content_parts,
			                     cost_details, usage_details, reasoning_details, request_settings)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
			RETURNING *
			"#,
		)
		.bind(chat_id)
		.bind(role)
		.bind(content)
		.bind(reasoning_content)
		.bind(model_id)
		.bind(parent_id)
		.bind(fork_index)
		.bind(content_parts)
		.bind(cost_details.map(sqlx::types::Json))
		.bind(usage_details.map(sqlx::types::Json))
		.bind(reasoning_details.map(sqlx::types::Json))
		.bind(sqlx::types::Json(request_settings.cloned().unwrap_or_default()))
		.fetch_one(conn)
		.await
	}

	pub async fn create_user_message(
		pool: &PgPool,
		chat_id: &Uuid,
		content: &str,
		model_id: Option<&Uuid>,
		reasoning_details: &ReasoningDetails,
	) -> Result<Self, sqlx::Error> {
		let cost_details = CostDetails::default();
		let usage_details = UsageDetails::default();
		let request_settings = RequestSettings::default();
		sqlx::query_as::<_, Message>(
			r#"
			INSERT INTO messages (chat_id, role, content, model_id, reasoning_details, usage_details, cost_details, request_settings)
			VALUES ($1, 'user', $2, $3, $4, $5, $6, $7)
			RETURNING *
			"#,
		)
		.bind(chat_id)
		.bind(content)
		.bind(model_id)
		.bind(sqlx::types::Json(reasoning_details))
		.bind(sqlx::types::Json(usage_details))
		.bind(sqlx::types::Json(cost_details))
		.bind(sqlx::types::Json(request_settings))
		.fetch_one(pool)
		.await
	}

	pub async fn next_fork_index(conn: &mut sqlx::PgConnection, chat_id: &Uuid, parent_id: Option<&Uuid>) -> Result<i32, sqlx::Error> {
		let row: (Option<i32>,) = sqlx::query_as("SELECT COALESCE(MAX(fork_index), 0) + 1 FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2")
			.bind(chat_id)
			.bind(parent_id)
			.fetch_one(conn)
			.await?;
		Ok(row.0.unwrap_or(1))
	}

	pub async fn sibling_count(pool: &PgPool, chat_id: &Uuid, parent_id: Option<&Uuid>) -> Result<i64, sqlx::Error> {
		let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2")
			.bind(chat_id)
			.bind(parent_id)
			.fetch_one(pool)
			.await?;
		Ok(row.0)
	}

	pub async fn siblings(pool: &PgPool, chat_id: &Uuid, parent_id: Option<&Uuid>) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2 ORDER BY fork_index")
			.bind(chat_id)
			.bind(parent_id)
			.fetch_all(pool)
			.await
	}

	pub async fn sibling_counts_for_chat(pool: &PgPool, chat_id: &Uuid) -> Result<HashMap<(Option<Uuid>, String), i64>, sqlx::Error> {
		let rows = sqlx::query_as::<_, SiblingCountRow>(
			r#"
			SELECT parent_id, role, COUNT(*) AS count
			FROM messages
			WHERE chat_id = $1
			GROUP BY parent_id, role
			"#,
		)
		.bind(chat_id)
		.fetch_all(pool)
		.await?;

		Ok(rows.into_iter().map(|r| ((r.parent_id, r.role), r.count)).collect())
	}

	pub async fn tool_executions_for_messages(pool: &PgPool, message_ids: &[Uuid]) -> Result<HashMap<Uuid, Vec<ToolExecutionResponse>>, sqlx::Error> {
		if message_ids.is_empty() {
			return Ok(HashMap::new());
		}

		let rows = sqlx::query_as::<_, ToolExecutionRow>(
			r#"
			SELECT te.id, te.message_id, te.tool_call_id, te.input_args, te.output, te.error, te.execution_ms, te.tool_id, te.tool_function, t.name AS tool_name
			FROM tool_executions te
			LEFT JOIN tools t ON te.tool_id = t.id
			WHERE te.message_id = ANY($1)
			ORDER BY te.created_at ASC
			"#,
		)
		.bind(message_ids)
		.fetch_all(pool)
		.await?;

		let mut by_message: HashMap<Uuid, Vec<ToolExecutionResponse>> = HashMap::new();
		for row in rows {
			if let Some(msg_id) = row.message_id {
				by_message.entry(msg_id).or_default().push(ToolExecutionResponse {
					tool_call_id: row.tool_call_id,
					tool_name: row.tool_name.unwrap_or_else(|| format!("tool_{}", row.id)),
					input_args: row.input_args,
					output: row.output,
					error: row.error,
					execution_ms: row.execution_ms,
					tool_id: row.tool_id,
					tool_function: row.tool_function,
				});
			}
		}
		Ok(by_message)
	}
}

impl UserPreferences {
	pub async fn find_by_user_id(pool: &PgPool, user_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, UserPreferences>("SELECT * FROM user_preferences WHERE user_id = $1")
			.bind(user_id)
			.fetch_optional(pool)
			.await
	}

	pub async fn upsert(pool: &PgPool, user_id: &Uuid, prefs: &UpdatePreferencesRequest) -> Result<Self, sqlx::Error> {
		fn json_or_null<T: serde::Serialize>(v: &Option<T>) -> serde_json::Value {
			v.as_ref()
				.map(|x| serde_json::to_value(x).unwrap_or(serde_json::Value::Null))
				.unwrap_or(serde_json::Value::Null)
		}

		sqlx::query_as::<_, UserPreferences>(
			r#"
			INSERT INTO user_preferences (user_id, default_model_key, favorite_model_keys,
			                              streaming_animation, use_remend, theme_css_vars, custom_theme_urls,
			                              default_provider_slug, default_tools)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
			ON CONFLICT (user_id) DO UPDATE
				SET default_model_key = CASE WHEN $10 THEN $2 ELSE user_preferences.default_model_key END,
				    favorite_model_keys = CASE WHEN $3::jsonb = 'null'::jsonb THEN user_preferences.favorite_model_keys ELSE $3::jsonb END,
				    streaming_animation = COALESCE($4, user_preferences.streaming_animation),
				    use_remend = COALESCE($5, user_preferences.use_remend),
				    theme_css_vars = CASE WHEN $6::jsonb = 'null'::jsonb THEN user_preferences.theme_css_vars ELSE $6::jsonb END,
				    custom_theme_urls = CASE WHEN $7::jsonb = 'null'::jsonb THEN user_preferences.custom_theme_urls ELSE $7::jsonb END,
				    default_provider_slug = CASE WHEN $11 THEN $8 ELSE user_preferences.default_provider_slug END,
				    default_tools = CASE WHEN $9::jsonb = 'null'::jsonb THEN user_preferences.default_tools ELSE $9::jsonb END,
				    updated_at = NOW()
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(prefs.default_model_key.as_ref().and_then(|v| v.as_deref()))
		.bind(json_or_null(&prefs.favorite_model_keys))
		.bind(&prefs.streaming_animation)
		.bind(prefs.use_remend)
		.bind(json_or_null(&prefs.theme_css_vars))
		.bind(json_or_null(&prefs.custom_theme_urls))
		.bind(prefs.default_provider_slug.as_ref().and_then(|v| v.as_deref()))
		.bind(json_or_null(&prefs.default_tools))
		.bind(prefs.default_model_key.is_some())
		.bind(prefs.default_provider_slug.is_some())
		.fetch_one(pool)
		.await
	}
}
