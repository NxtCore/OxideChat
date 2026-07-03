use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{Chat, CreateWorkspaceRequest, Message, UpdateWorkspaceRequest, Workspace, WorkspaceDeleteAction};

impl Workspace {
	pub async fn create_from_request(pool: &PgPool, user_id: &Uuid, req: &CreateWorkspaceRequest) -> Result<Self, sqlx::Error> {
		if !req.is_default {
			return Self::create(pool, user_id, &req.name, req.icon.as_deref(), req.color.as_deref(), req.is_default).await;
		}

		let mut tx = pool.begin().await?;
		Self::clear_default_for_user_tx(&mut tx, user_id, None).await?;
		let workspace = Self::create_tx(&mut tx, user_id, req).await?;
		tx.commit().await?;
		Ok(workspace)
	}

	async fn create_tx(tx: &mut Transaction<'_, Postgres>, user_id: &Uuid, req: &CreateWorkspaceRequest) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, Workspace>(
			r#"
			INSERT INTO workspaces (user_id, name, icon, color, is_default)
			VALUES ($1, $2, $3, $4, $5)
			RETURNING *
			"#,
		)
		.bind(user_id)
		.bind(&req.name)
		.bind(req.icon.as_deref())
		.bind(req.color.as_deref())
		.bind(req.is_default)
		.fetch_one(&mut **tx)
		.await
	}

	async fn clear_default_for_user_tx(tx: &mut Transaction<'_, Postgres>, user_id: &Uuid, exclude_id: Option<&Uuid>) -> Result<(), sqlx::Error> {
		if let Some(exclude) = exclude_id {
			sqlx::query("UPDATE workspaces SET is_default = false WHERE user_id = $1 AND id != $2")
				.bind(user_id)
				.bind(exclude)
				.execute(&mut **tx)
				.await?;
		} else {
			sqlx::query("UPDATE workspaces SET is_default = false WHERE user_id = $1")
				.bind(user_id)
				.execute(&mut **tx)
				.await?;
		}
		Ok(())
	}

	pub async fn update(
		pool: &PgPool,
		id: &Uuid,
		user_id: &Uuid,
		name: Option<&str>,
		icon: Option<&str>,
		color: Option<Option<&str>>,
		sort_order: Option<i32>,
		is_default: Option<bool>,
	) -> Result<Option<Self>, sqlx::Error> {
		let update_color = color.is_some();
		let color_value = color.flatten();

		sqlx::query_as::<_, Workspace>(
			r#"
			UPDATE workspaces
			SET name = COALESCE($3, name),
			    icon = COALESCE($4, icon),
			    color = CASE WHEN $8 THEN $5 ELSE color END,
			    sort_order = COALESCE($6, sort_order),
			    is_default = COALESCE($7, is_default),
			    updated_at = NOW()
			WHERE id = $1 AND user_id = $2
			RETURNING *
			"#,
		)
		.bind(id)
		.bind(user_id)
		.bind(name)
		.bind(icon)
		.bind(color_value)
		.bind(sort_order)
		.bind(is_default)
		.bind(update_color)
		.fetch_optional(pool)
		.await
	}

	async fn update_tx(tx: &mut Transaction<'_, Postgres>, id: &Uuid, user_id: &Uuid, req: &UpdateWorkspaceRequest) -> Result<Option<Self>, sqlx::Error> {
		let update_color = req.color.is_some();
		let color_value = req.color.as_ref().and_then(Option::as_deref);

		sqlx::query_as::<_, Workspace>(
			r#"
			UPDATE workspaces
			SET name = COALESCE($3, name),
			    icon = COALESCE($4, icon),
			    color = CASE WHEN $8 THEN $5 ELSE color END,
			    sort_order = COALESCE($6, sort_order),
			    is_default = COALESCE($7, is_default),
			    updated_at = NOW()
			WHERE id = $1 AND user_id = $2
			RETURNING *
			"#,
		)
		.bind(id)
		.bind(user_id)
		.bind(req.name.as_deref())
		.bind(req.icon.as_deref())
		.bind(color_value)
		.bind(req.sort_order)
		.bind(req.is_default)
		.bind(update_color)
		.fetch_optional(&mut **tx)
		.await
	}

	pub async fn update_from_request(pool: &PgPool, id: &Uuid, user_id: &Uuid, req: &UpdateWorkspaceRequest) -> Result<Option<Self>, sqlx::Error> {
		if req.is_default != Some(true) {
			return Self::update(
				pool,
				id,
				user_id,
				req.name.as_deref(),
				req.icon.as_deref(),
				req.color.as_ref().map(Option::as_deref),
				req.sort_order,
				req.is_default,
			)
			.await;
		}

		let mut tx = pool.begin().await?;
		Self::clear_default_for_user_tx(&mut tx, user_id, Some(id)).await?;
		let updated = Self::update_tx(&mut tx, id, user_id, req).await?;
		if updated.is_some() {
			tx.commit().await?;
		}
		Ok(updated)
	}

	async fn delete_tx(tx: &mut Transaction<'_, Postgres>, id: &Uuid, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM workspaces WHERE id = $1 AND user_id = $2")
			.bind(id)
			.bind(user_id)
			.execute(&mut **tx)
			.await?;
		Ok(result.rows_affected() > 0)
	}

	pub async fn delete_with_chat_disposition(
		pool: &PgPool,
		id: &Uuid,
		user_id: &Uuid,
		action: WorkspaceDeleteAction,
		target_workspace_id: Option<&Uuid>,
	) -> Result<bool, sqlx::Error> {
		let mut tx = pool.begin().await?;
		match action {
			WorkspaceDeleteAction::Move => {
				if let Some(target) = target_workspace_id {
					Chat::move_all_to_workspace_tx(&mut tx, user_id, id, target).await?;
				}
			}
			WorkspaceDeleteAction::Archive => {
				Chat::archive_all_in_workspace_tx(&mut tx, user_id, id).await?;
			}
			WorkspaceDeleteAction::Delete => {
				Chat::delete_all_in_workspace_tx(&mut tx, user_id, id).await?;
			}
		}
		let deleted = Self::delete_tx(&mut tx, id, user_id).await?;
		if deleted {
			tx.commit().await?;
		}
		Ok(deleted)
	}
}

impl Chat {
	pub async fn update(
		pool: &PgPool,
		id: &Uuid,
		user_id: &Uuid,
		title: Option<&str>,
		workspace_id: Option<Option<&Uuid>>,
		is_pinned: Option<bool>,
		is_archived: Option<bool>,
	) -> Result<Option<Self>, sqlx::Error> {
		let update_workspace = workspace_id.is_some();
		let workspace_value = workspace_id.flatten();

		sqlx::query_as::<_, Chat>(
			r#"
			UPDATE chats
			SET title = COALESCE($3, title),
			    workspace_id = CASE WHEN $7 THEN $4 ELSE workspace_id END,
			    is_pinned = COALESCE($5, is_pinned),
			    is_archived = COALESCE($6, is_archived),
			    updated_at = NOW()
			WHERE id = $1 AND user_id = $2
			RETURNING *
			"#,
		)
		.bind(id)
		.bind(user_id)
		.bind(title)
		.bind(workspace_value)
		.bind(is_pinned)
		.bind(is_archived)
		.bind(update_workspace)
		.fetch_optional(pool)
		.await
	}

	pub async fn touch(pool: &PgPool, id: &Uuid) -> Result<(), sqlx::Error> {
		sqlx::query("UPDATE chats SET updated_at = NOW() WHERE id = $1").bind(id).execute(pool).await?;
		Ok(())
	}

	async fn move_all_to_workspace_tx(tx: &mut Transaction<'_, Postgres>, user_id: &Uuid, from: &Uuid, to: &Uuid) -> Result<u64, sqlx::Error> {
		let result = sqlx::query("UPDATE chats SET workspace_id = $3, updated_at = NOW() WHERE user_id = $1 AND workspace_id = $2")
			.bind(user_id)
			.bind(from)
			.bind(to)
			.execute(&mut **tx)
			.await?;
		Ok(result.rows_affected())
	}

	async fn archive_all_in_workspace_tx(tx: &mut Transaction<'_, Postgres>, user_id: &Uuid, workspace_id: &Uuid) -> Result<u64, sqlx::Error> {
		let result = sqlx::query("UPDATE chats SET is_archived = true, workspace_id = NULL, updated_at = NOW() WHERE user_id = $1 AND workspace_id = $2")
			.bind(user_id)
			.bind(workspace_id)
			.execute(&mut **tx)
			.await?;
		Ok(result.rows_affected())
	}

	async fn delete_all_in_workspace_tx(tx: &mut Transaction<'_, Postgres>, user_id: &Uuid, workspace_id: &Uuid) -> Result<u64, sqlx::Error> {
		let result = sqlx::query("DELETE FROM chats WHERE user_id = $1 AND workspace_id = $2")
			.bind(user_id)
			.bind(workspace_id)
			.execute(&mut **tx)
			.await?;
		Ok(result.rows_affected())
	}

	pub async fn delete(pool: &PgPool, id: &Uuid, user_id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM chats WHERE id = $1 AND user_id = $2")
			.bind(id)
			.bind(user_id)
			.execute(pool)
			.await?;
		Ok(result.rows_affected() > 0)
	}
}

impl Message {
	/// Deactivate all siblings at the given parent_id level and their entire subtrees.
	pub async fn deactivate_fork_level(pool: &PgPool, chat_id: &Uuid, parent_id: Option<&Uuid>) -> Result<(), sqlx::Error> {
		sqlx::query(
			r#"
			WITH RECURSIVE descendants AS (
				SELECT id FROM messages
				WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2
				UNION ALL
				SELECT m.id FROM messages m
				INNER JOIN descendants d ON m.parent_id = d.id
				WHERE m.chat_id = $1
			)
			UPDATE messages SET is_active_fork = FALSE
			WHERE id IN (SELECT id FROM descendants)
			"#,
		)
		.bind(chat_id)
		.bind(parent_id)
		.execute(pool)
		.await?;
		Ok(())
	}

	/// Create a new fork of the given message with different content.
	/// Deactivates all current siblings and their subtrees, then inserts the new fork.
	pub async fn create_fork(pool: &PgPool, chat_id: &Uuid, original: &Message, new_content: &str) -> Result<Self, sqlx::Error> {
		let next_fork_index: i32 = {
			let row: (Option<i32>,) = sqlx::query_as("SELECT COALESCE(MAX(fork_index), 0) + 1 FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2")
				.bind(chat_id)
				.bind(original.parent_id)
				.fetch_one(pool)
				.await?;
			row.0.unwrap_or(1)
		};

		Self::deactivate_fork_level(pool, chat_id, original.parent_id.as_ref()).await?;

		sqlx::query_as::<_, Message>(
			r#"
			INSERT INTO messages (chat_id, role, content, model_id, reasoning_details, usage_details, cost_details, parent_id, fork_index, is_active_fork)
			VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, TRUE)
			RETURNING *
			"#,
		)
		.bind(chat_id)
		.bind(&original.role)
		.bind(new_content)
		.bind(original.model_id)
		.bind(&original.reasoning_details)
		.bind(&original.usage_details)
		.bind(&original.cost_details)
		.bind(original.parent_id)
		.bind(next_fork_index)
		.fetch_one(pool)
		.await
	}

	/// Switch the active fork at the given parent_id level to the specified fork_index.
	/// Deactivates the currently active subtree and activates the target + its descendants (following fork_index = 1).
	pub async fn switch_to_fork(pool: &PgPool, chat_id: &Uuid, parent_id: Option<&Uuid>, fork_index: i32) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query(
			r#"
			WITH RECURSIVE descendants AS (
				SELECT id FROM messages
				WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND is_active_fork = TRUE
				UNION ALL
				SELECT m.id FROM messages m
				INNER JOIN descendants d ON m.parent_id = d.id
				WHERE m.chat_id = $1
			)
			UPDATE messages SET is_active_fork = FALSE
			WHERE id IN (SELECT id FROM descendants)
			"#,
		)
		.bind(chat_id)
		.bind(parent_id)
		.execute(pool)
		.await?;

		let target = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE chat_id = $1 AND parent_id IS NOT DISTINCT FROM $2 AND fork_index = $3")
			.bind(chat_id)
			.bind(parent_id)
			.bind(fork_index)
			.fetch_optional(pool)
			.await?;

		if let Some(ref msg) = target {
			sqlx::query(
				r#"
				WITH RECURSIVE descendants AS (
					SELECT id FROM messages WHERE id = $1
					UNION ALL
					SELECT m.id FROM messages m
					INNER JOIN descendants d ON m.parent_id = d.id
					WHERE m.chat_id = $2 AND m.fork_index = 1
				)
				UPDATE messages SET is_active_fork = TRUE
				WHERE id IN (SELECT id FROM descendants)
				"#,
			)
			.bind(msg.id)
			.bind(chat_id)
			.execute(pool)
			.await?;
		}

		Ok(target)
	}

	pub async fn copy_to_chat(pool: &PgPool, new_chat_id: &Uuid, messages: &[Message]) -> Result<std::collections::HashMap<Uuid, Uuid>, sqlx::Error> {
		let mut old_to_new: std::collections::HashMap<Uuid, Uuid> = std::collections::HashMap::new();

		for msg in messages {
			let new_parent_id = msg.parent_id.and_then(|pid| old_to_new.get(&pid).copied());

			let new_msg = sqlx::query_as::<_, Message>(
				r#"
				INSERT INTO messages (chat_id, role, content, content_parts, reasoning_content, model_id,
					cost_details, usage_details, reasoning_details, parent_id, fork_index, is_active_fork)
				VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1, TRUE)
				RETURNING *
				"#,
			)
			.bind(new_chat_id)
			.bind(&msg.role)
			.bind(&msg.content)
			.bind(&msg.content_parts)
			.bind(&msg.reasoning_content)
			.bind(msg.model_id)
			.bind(&msg.cost_details)
			.bind(&msg.usage_details)
			.bind(&msg.reasoning_details)
			.bind(new_parent_id)
			.fetch_one(pool)
			.await?;

			old_to_new.insert(msg.id, new_msg.id);
		}

		Ok(old_to_new)
	}

	pub async fn delete(pool: &PgPool, id: &Uuid, chat_id: &Uuid) -> Result<bool, sqlx::Error> {
		let result = sqlx::query("DELETE FROM messages WHERE id = $1 AND chat_id = $2")
			.bind(id)
			.bind(chat_id)
			.execute(pool)
			.await?;
		Ok(result.rows_affected() > 0)
	}
}
