#[cfg(test)]
mod tests {
	use crate::types::{Chat, Workspace};
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

	async fn insert_message(pool: &PgPool, chat_id: &Uuid) {
		sqlx::query("INSERT INTO messages (chat_id, role, content) VALUES ($1, 'user', 'hi')")
			.bind(chat_id)
			.execute(pool)
			.await
			.unwrap();
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn move_disposition_reassigns_chats(pool: PgPool) {
		let user = create_user(&pool, "move").await;
		let from = Workspace::create(&pool, &user, "From", None, None, false).await.unwrap();
		let to = Workspace::create(&pool, &user, "To", None, None, false).await.unwrap();
		let chat = Chat::create(&pool, &user, Some(&from.id), Some("c")).await.unwrap();

		let moved = Chat::move_all_to_workspace(&pool, &user, &from.id, &to.id).await.unwrap();
		assert_eq!(moved, 1);

		let reloaded = Chat::find_by_id_and_user(&pool, &chat.id, &user).await.unwrap().unwrap();
		assert_eq!(reloaded.workspace_id, Some(to.id));

		assert!(Workspace::delete(&pool, &from.id, &user).await.unwrap());
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn archive_disposition_archives_and_unassigns(pool: PgPool) {
		let user = create_user(&pool, "archive").await;
		let ws = Workspace::create(&pool, &user, "Ws", None, None, false).await.unwrap();
		let chat = Chat::create(&pool, &user, Some(&ws.id), Some("c")).await.unwrap();

		let archived = Chat::archive_all_in_workspace(&pool, &user, &ws.id).await.unwrap();
		assert_eq!(archived, 1);

		let reloaded = Chat::find_by_id_and_user(&pool, &chat.id, &user).await.unwrap().unwrap();
		assert!(reloaded.is_archived);
		assert_eq!(reloaded.workspace_id, None);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn update_color_clears_only_on_explicit_null(pool: PgPool) {
		let user = create_user(&pool, "color").await;
		let ws = Workspace::create(&pool, &user, "Ws", None, Some("#3b82f6"), false).await.unwrap();

		let kept = Workspace::update(&pool, &ws.id, &user, Some("Renamed"), None, None, None, None)
			.await
			.unwrap()
			.unwrap();
		assert_eq!(kept.color.as_deref(), Some("#3b82f6"));

		let recolored = Workspace::update(&pool, &ws.id, &user, None, None, Some(Some("#ef4444")), None, None)
			.await
			.unwrap()
			.unwrap();
		assert_eq!(recolored.color.as_deref(), Some("#ef4444"));

		let cleared = Workspace::update(&pool, &ws.id, &user, None, None, Some(None), None, None).await.unwrap().unwrap();
		assert_eq!(cleared.color, None);
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn delete_disposition_removes_chats_and_messages(pool: PgPool) {
		let user = create_user(&pool, "delete").await;
		let ws = Workspace::create(&pool, &user, "Ws", None, None, false).await.unwrap();
		let chat = Chat::create(&pool, &user, Some(&ws.id), Some("c")).await.unwrap();
		insert_message(&pool, &chat.id).await;

		let deleted = Chat::delete_all_in_workspace(&pool, &user, &ws.id).await.unwrap();
		assert_eq!(deleted, 1);

		assert!(Chat::find_by_id_and_user(&pool, &chat.id, &user).await.unwrap().is_none());
		let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE chat_id = $1")
			.bind(chat.id)
			.fetch_one(&pool)
			.await
			.unwrap();
		assert_eq!(remaining, 0);
	}
}
