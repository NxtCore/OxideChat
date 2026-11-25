#[cfg(test)]
mod extraction {
	use crate::types::AttachmentError;
	use crate::utils::extract::{DocumentKind, extract, parse};

	const PDF: &[u8] = include_bytes!("fixtures/two_pages.pdf");

	#[test]
	fn the_kind_follows_the_extension_and_a_pdf_must_look_like_one() {
		assert_eq!(DocumentKind::detect("notes.MD", b"# hi"), Some(DocumentKind::Text));
		assert_eq!(DocumentKind::detect("page.html", b"<p>"), Some(DocumentKind::Html));
		assert_eq!(DocumentKind::detect("report.pdf", PDF), Some(DocumentKind::Pdf));
		assert_eq!(DocumentKind::detect("report.pdf", b"not a pdf"), None);
		assert_eq!(DocumentKind::detect("program.exe", b"MZ"), None);
		assert_eq!(DocumentKind::detect("no_extension", b"text"), None);
	}

	#[test]
	fn pdf_pages_are_marked() {
		let text = parse(DocumentKind::Pdf, PDF).unwrap_or_default();
		assert!(text.starts_with("<!-- page 1 -->\nHello page one"), "{text}");
		assert!(text.contains("<!-- page 2 -->\nSecond page text"), "{text}");
	}

	#[test]
	fn html_keeps_headings_and_loads_nothing() {
		let text = parse(DocumentKind::Html, br#"<h1>Title</h1><p>Body <img src="https://example.com/x.png" alt="pic"></p>"#).unwrap_or_default();
		assert!(text.starts_with("# Title"), "{text}");
		assert!(text.contains("Body"), "{text}");
	}

	#[tokio::test]
	async fn text_files_must_be_text_and_not_empty() {
		assert_eq!(extract(DocumentKind::Text, b"\xEF\xBB\xBF  hello \n").await.ok().as_deref(), Some("hello"));
		assert!(matches!(extract(DocumentKind::Text, b"a\0b").await, Err(AttachmentError::NotText)));
		assert!(matches!(extract(DocumentKind::Text, b"\xff\xfe").await, Err(AttachmentError::NotText)));
		assert!(matches!(extract(DocumentKind::Text, b" \n ").await, Err(AttachmentError::Empty)));
	}
}

#[cfg(test)]
mod storage {
	use crate::tests::support::{TestResult, insert_user};
	use crate::types::chat::runner::prepare::{attachment_bytes, fit_context};
	use crate::types::models::Model;
	use crate::types::{
		AttachmentError, Chat, ChatAttachment, CostDetails, Message, ReasoningDetails, RequestSettings, StreamRequest, StreamingUserMessageCreate, UsageDetails,
	};
	use serde_json::json;
	use sqlx::PgPool;
	use sqlx::types::Json;

	fn request(parts: serde_json::Value) -> Result<StreamRequest, serde_json::Error> {
		serde_json::from_value(json!({"content": "summarize", "model_key": "m", "parts": parts}))
	}

	fn model(context_length: Option<i32>) -> Model {
		Model {
			id: uuid::Uuid::new_v4(),
			provider_id: uuid::Uuid::new_v4(),
			model_id: "test/model".to_string(),
			display_name: "Test".to_string(),
			capabilities: Json(vec![]),
			input_modalities: Json(vec![]),
			output_modalities: Json(vec![]),
			context_length,
			max_tokens: None,
			is_enabled: true,
			created_at: chrono::Utc::now(),
			updated_at: chrono::Utc::now(),
		}
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn uploads_are_checked_and_only_their_owner_may_attach_them(pool: PgPool) -> TestResult {
		crate::config::Config::init(&pool).await?;
		crate::i18n::I18n::init(&pool).await;
		let owner = insert_user(&pool, "attach_owner").await?;
		let stranger = insert_user(&pool, "attach_stranger").await?;
		assert!(matches!(
			ChatAttachment::upload(&pool, &owner, "tool.exe", b"MZ").await,
			Err(AttachmentError::UnsupportedType)
		));
		assert!(matches!(
			ChatAttachment::upload(&pool, &owner, " ", b"x").await,
			Err(AttachmentError::InvalidFilename)
		));

		let attachment = ChatAttachment::upload(&pool, &owner, "notes.txt", b"The launch is on Monday.").await?;
		assert_eq!(attachment.text, "The launch is on Monday.");
		assert_eq!(attachment.mime_type, "text/plain");
		assert!(attachment.token_count > 0);

		let req = request(json!([{"type": "file", "attachment_id": attachment.id, "filename": "notes.txt"}]))?;
		let bytes = attachment_bytes(&pool, &owner, &req).await.map_err(|error| format!("{error:?}"))?;
		assert_eq!(bytes, attachment.prompt_text().len() as u64);
		assert!(attachment_bytes(&pool, &stranger, &req).await.is_err());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_history_estimate_counts_attached_text(pool: PgPool) -> TestResult {
		let user_id = insert_user(&pool, "attach_history").await?;
		let chat = Chat::create(&pool, &user_id, None, None).await?;
		let empty = Message::active_prompt_bytes(&pool, &chat.id).await?;
		let attachment = ChatAttachment::upload(&pool, &user_id, "data.csv", "a,b\n1,2\n".repeat(100).as_bytes()).await?;
		let parts = json!([{"type": "file", "attachment_id": attachment.id, "filename": "data.csv"}]);
		Message::create_streaming_user(
			&pool,
			StreamingUserMessageCreate {
				chat_id: &chat.id,
				content: "",
				content_parts: Some(&parts),
				model_id: None,
				reasoning_details: &ReasoningDetails::default(),
				usage_details: &UsageDetails::default(),
				cost_details: &CostDetails::default(),
				request_settings: &RequestSettings::default(),
				parent_id: None,
			},
		)
		.await?;
		let added = Message::active_prompt_bytes(&pool, &chat.id).await? - empty;
		let text = attachment.text.len() as u64;
		assert!(added > text && added < text + 200, "{added}");
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn only_turns_with_attachments_are_refused_for_the_context_window(pool: PgPool) -> TestResult {
		crate::config::Config::init(&pool).await?;
		crate::i18n::I18n::init(&pool).await;
		let small = model(Some(100));
		assert!(fit_context(&small, None, 100, 400).is_err());
		assert_eq!(fit_context(&small, None, 100, 200).ok(), Some(300));
		assert_eq!(fit_context(&small, None, 4000, 0).ok(), Some(4000));
		assert_eq!(fit_context(&model(None), None, 100, 4000).ok(), Some(4100));
		Ok(())
	}
}
