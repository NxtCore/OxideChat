#[cfg(test)]
mod sampling {
	use crate::types::ChatSampling;
	use serde_json::json;

	fn sampling(value: serde_json::Value) -> ChatSampling {
		serde_json::from_value(value).unwrap_or_default()
	}

	#[test]
	fn empty_and_in_range_values_are_valid() {
		assert!(ChatSampling::default().is_valid());
		let valid = sampling(json!({
			"temperature": 2.0,
			"top_p": 0.0,
			"max_tokens": 1,
			"frequency_penalty": -2.0,
			"presence_penalty": 2.0,
			"seed": -7,
			"custom": {"min_p": 0.05, "top_a": 0.1, "provider.option-x": {"nested": true}}
		}));
		assert!(valid.is_valid());
	}

	#[test]
	fn out_of_range_values_are_rejected() {
		for value in [
			json!({"temperature": 2.5}),
			json!({"temperature": -0.1}),
			json!({"top_p": 1.5}),
			json!({"frequency_penalty": -3}),
			json!({"presence_penalty": 2.1}),
			json!({"max_tokens": 0}),
		] {
			assert!(!sampling(value.clone()).is_valid(), "{value}");
		}
	}

	#[test]
	fn reserved_or_malformed_custom_keys_are_rejected() {
		for key in ["model", "Model", "messages", "stream", "tools", "", "has space", "quote\"key"] {
			let value = sampling(json!({"custom": {key: 1}}));
			assert!(!value.is_valid(), "{key}");
		}
		let long_key = "k".repeat(65);
		assert!(!sampling(json!({"custom": {long_key: 1}})).is_valid());
	}

	#[test]
	fn too_many_or_too_large_custom_fields_are_rejected() {
		let many: serde_json::Map<String, serde_json::Value> = (0..33).map(|i| (format!("field_{i}"), json!(i))).collect();
		assert!(!ChatSampling { custom: many, ..Default::default() }.is_valid());
		let large = sampling(json!({"custom": {"blob": "x".repeat(17 * 1024)}}));
		assert!(!large.is_valid());
	}

	#[test]
	fn serializes_without_unset_fields() -> Result<(), serde_json::Error> {
		let value = serde_json::to_value(sampling(json!({"temperature": 0.5})))?;
		assert_eq!(value, json!({"temperature": 0.5}));
		Ok(())
	}
}

#[cfg(test)]
mod merge {
	use crate::types::ChatSampling;
	use crate::types::chat::runner::request::merge_sampling_with_priority;
	use crate::types::models::Model;
	use serde_json::json;
	use sqlx::types::Json;

	fn model(max_tokens: Option<i32>) -> Model {
		Model {
			id: uuid::Uuid::new_v4(),
			provider_id: uuid::Uuid::new_v4(),
			model_id: "test/model".to_string(),
			display_name: "Test".to_string(),
			capabilities: Json(vec![]),
			input_modalities: Json(vec![]),
			output_modalities: Json(vec![]),
			context_length: None,
			max_tokens,
			is_enabled: true,
			created_at: chrono::Utc::now(),
			updated_at: chrono::Utc::now(),
		}
	}

	#[test]
	fn chat_values_apply_below_request_values() -> Result<(), serde_json::Error> {
		let chat: ChatSampling = serde_json::from_value(json!({"temperature": 0.3, "top_p": 0.9, "max_tokens": 200, "seed": 7}))?;
		let request: omniference::Sampling = serde_json::from_value(json!({"temperature": 1.1, "stop": []}))?;
		let merged = merge_sampling_with_priority(Some(&request), &chat, None, &model(Some(4096)));
		assert_eq!(merged.temperature, Some(1.1));
		assert_eq!(merged.top_p, Some(0.9));
		assert_eq!(merged.max_tokens, Some(200));
		assert_eq!(merged.seed, Some(7));
		Ok(())
	}

	#[test]
	fn an_empty_chat_keeps_the_model_defaults() {
		let merged = merge_sampling_with_priority(None, &ChatSampling::default(), None, &model(Some(4096)));
		assert_eq!(merged.max_tokens, Some(4096));
		assert_eq!(merged.temperature, None);
	}
}

#[cfg(test)]
mod storage {
	use crate::tests::support::insert_user;
	use crate::types::{Chat, ChatSampling, Message};
	use serde_json::json;
	use sqlx::PgPool;

	type TestResult = Result<(), Box<dyn std::error::Error>>;

	#[sqlx::test(migrations = "./migrations")]
	async fn sampling_is_saved_per_chat_and_copied_to_branches(pool: PgPool) -> TestResult {
		let user_id = insert_user(&pool, "sampling").await?;
		let chat = Chat::create(&pool, &user_id, None, Some("Sampling")).await?;
		let other = Chat::create(&pool, &user_id, None, Some("Other")).await?;
		assert_eq!(chat.sampling.0, ChatSampling::default());

		let sampling: ChatSampling = serde_json::from_value(json!({"temperature": 0.4, "custom": {"min_p": 0.1}}))?;
		let saved = Chat::set_sampling(&pool, &chat.id, &user_id, &sampling).await?.ok_or("chat missing")?;
		assert_eq!(saved.sampling.0, sampling);
		assert_eq!(saved.updated_at, chat.updated_at);

		let reloaded = Chat::find_by_id_and_user(&pool, &chat.id, &user_id).await?.ok_or("chat missing")?;
		assert_eq!(reloaded.sampling.0, sampling);
		let untouched = Chat::find_by_id_and_user(&pool, &other.id, &user_id).await?.ok_or("chat missing")?;
		assert_eq!(untouched.sampling.0, ChatSampling::default());

		let message = Message::create_user_message(&pool, &chat.id, "hi", None, &crate::types::ReasoningDetails::default()).await?;
		let branch = Chat::create_branched(&pool, &user_id, None, Some("Branch"), &chat.id, &message.id).await?;
		assert_eq!(branch.sampling.0, sampling);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn other_users_cannot_change_a_chats_sampling(pool: PgPool) -> TestResult {
		let owner = insert_user(&pool, "owner").await?;
		let stranger = insert_user(&pool, "stranger").await?;
		let chat = Chat::create(&pool, &owner, None, None).await?;
		let sampling: ChatSampling = serde_json::from_value(json!({"temperature": 1.0}))?;
		assert!(Chat::set_sampling(&pool, &chat.id, &stranger, &sampling).await?.is_none());
		let reloaded = Chat::find_by_id_and_user(&pool, &chat.id, &owner).await?.ok_or("chat missing")?;
		assert_eq!(reloaded.sampling.0, ChatSampling::default());
		Ok(())
	}
}
