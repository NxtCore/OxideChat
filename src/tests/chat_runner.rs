#[cfg(test)]
mod pending_calls {
	use crate::types::ClientToolPending;
	use std::time::Duration;
	use uuid::Uuid;

	#[tokio::test]
	async fn a_reply_that_arrives_right_after_registration_is_not_lost() {
		let pending = ClientToolPending::new();
		let user = Uuid::new_v4();
		let mut call = pending.register("call-1".to_string(), user).await;
		assert!(pending.resolve("call-1", user, serde_json::json!({"ok": true})).await);
		let result = call.wait(Duration::from_secs(1)).await;
		assert_eq!(result, Some(serde_json::json!({"ok": true})));
	}

	#[tokio::test]
	async fn dropping_the_call_removes_the_registration() {
		let pending = ClientToolPending::new();
		let user = Uuid::new_v4();
		let call = pending.register("call-1".to_string(), user).await;
		drop(call);
		assert!(!pending.resolve("call-1", user, serde_json::json!({})).await);
	}

	#[tokio::test]
	async fn waiting_times_out_without_a_reply() {
		let pending = ClientToolPending::new();
		let mut call = pending.register("call-1".to_string(), Uuid::new_v4()).await;
		assert_eq!(call.wait(Duration::from_millis(10)).await, None);
	}
}

#[cfg(test)]
mod agent {
	use crate::tests::gateway::support::{TestResult, create_provider_model, create_team, create_user, job_state};
	use crate::types::chat::runner::prepare::PreparedTurn;
	use crate::types::chat::runner::agent::{AgentRun, Finish, ToolOutput};
	use crate::types::*;
	use omniference::stream::StreamEvent;
	use sqlx::PgPool;
	use std::sync::Arc;
	use std::time::Instant;
	use uuid::Uuid;

	struct Fixture {
		state: Arc<JobState>,
		user_id: Uuid,
		chat_id: Uuid,
		model_id: Uuid,
		provider_id: Uuid,
	}

	async fn fixture(pool: &PgPool) -> TestResult<Fixture> {
		let user_id = create_user(pool).await?;
		create_team(pool, user_id, "Chat Team", true).await?;
		let (provider_id, model_id) = create_provider_model(pool, "Chat Provider", "chat-model").await?;
		sqlx::query("INSERT INTO model_pricing_overrides (model_id, pricing) VALUES ($1, $2)")
			.bind(model_id)
			.bind(serde_json::json!({ "input": 2.0, "output": 2.0 }))
			.execute(pool)
			.await?;
		let chat = Chat::create(pool, &user_id, None, Some("Runner")).await?;
		Ok(Fixture {
			state: job_state(pool.clone()),
			user_id,
			chat_id: chat.id,
			model_id,
			provider_id,
		})
	}

	fn run(fixture: &Fixture) -> AgentRun {
		let usage = ChatUsageGuard::new(Arc::clone(&fixture.state), None, fixture.user_id, fixture.model_id, fixture.provider_id);
		let turn = PreparedTurn {
			chat_id: fixture.chat_id,
			model_id: fixture.model_id,
			usage,
			user_id: fixture.user_id,
			user_message: None,
			ir: omniference::types::ChatRequestIR::default(),
			assistant_parent_id: None,
			assistant_fork_index: 1,
			reasoning_effort: None,
			reasoning_budget_tokens: None,
			request_settings: RequestSettings::default(),
		};
		AgentRun::new(Arc::clone(&fixture.state), turn, Instant::now())
	}

	fn cost(total: f64) -> StreamEvent {
		StreamEvent::Cost {
			cost: omniference::stream::CostDetails {
				total,
				prompt: None,
				completion: None,
				reasoning: None,
			},
		}
	}

	fn tool_output() -> ToolOutput {
		ToolOutput {
			output: serde_json::json!({"result": 1}),
			error: None,
			tool_id: None,
			function_id: None,
		}
	}

	fn done_message(finish: Finish) -> TestResult<ChatMessageResponse> {
		match finish {
			Finish::Stop(StreamData::Done { message }) => Ok(message),
			_ => Err("expected a finished turn".into()),
		}
	}

	async fn usage_cost(pool: &PgPool, user_id: Uuid) -> TestResult<(i64, rust_decimal::Decimal)> {
		Ok(sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(cost_total), 0) FROM usage_events WHERE user_id = $1")
			.bind(user_id)
			.fetch_one(pool)
			.await?)
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn events_update_the_stream_and_the_token_totals(pool: PgPool) -> TestResult {
		let mut run = run(&fixture(&pool).await?);
		let text = run.apply(StreamEvent::TextDelta { content: "Hi".to_string() });
		assert!(matches!(text, Some(StreamData::TextDelta { content }) if content == "Hi"));
		run.apply(StreamEvent::Tokens { input: 10, output: 5 });
		let tokens = run.apply(StreamEvent::Tokens { input: 1, output: 2 });
		assert!(matches!(tokens, Some(StreamData::Tokens { input: 11, output: 7, reasoning: None })));
		assert!(run.apply(StreamEvent::Done).is_none());
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_turn_without_tools_saves_the_message_and_records_usage(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let mut run = run(&fixture);
		run.next_request().map_err(|_| "request refused")?;
		run.apply(StreamEvent::TextDelta { content: "Hello ".to_string() });
		run.apply(StreamEvent::TextDelta { content: "world".to_string() });
		run.apply(StreamEvent::Tokens { input: 1_000_000, output: 0 });
		let message = done_message(run.finish_iteration().await)?;
		assert_eq!(message.content, "Hello world");
		assert_eq!(usage_cost(&pool, fixture.user_id).await?, (1, rust_decimal::Decimal::from(2)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_tool_step_continues_the_loop_and_sums_provider_cost(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let mut run = run(&fixture);
		run.next_request().map_err(|_| "request refused")?;
		run.apply(StreamEvent::ToolCallStart {
			id: "call-1".to_string(),
			name: "lookup".to_string(),
			args_json: serde_json::json!(""),
		});
		run.apply(StreamEvent::ToolCallDelta {
			id: "call-1".to_string(),
			args_delta_json: serde_json::json!("{}"),
		});
		run.apply(StreamEvent::Tokens { input: 1_000_000, output: 0 });
		run.apply(cost(0.25));
		assert!(run.take_pending_call("unknown".to_string(), serde_json::json!({})).is_none());
		let call = run.take_pending_call("call-1".to_string(), serde_json::json!({})).ok_or("missing call")?;
		let result = run.record_tool_result(call, Instant::now(), tool_output());
		assert!(matches!(result, StreamData::ToolResult { id, .. } if id == "call-1"));
		assert!(matches!(run.finish_iteration().await, Finish::Continue));

		let request = run.next_request().map_err(|_| "request refused")?;
		assert_eq!(request.messages.len(), 2);
		run.apply(StreamEvent::TextDelta { content: "Done".to_string() });
		run.apply(StreamEvent::Tokens { input: 1_000_000, output: 0 });
		run.apply(cost(0.25));
		let message = done_message(run.finish_iteration().await)?;
		assert_eq!(message.content, "Done");
		let parts = message.content_parts.ok_or("missing parts")?;
		assert!(parts.to_string().contains("tool_call"));
		assert_eq!(usage_cost(&pool, fixture.user_id).await?, (1, "0.5".parse()?));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn an_iteration_without_cost_prices_all_tokens_locally(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let mut run = run(&fixture);
		run.next_request().map_err(|_| "request refused")?;
		run.apply(StreamEvent::Tokens { input: 1_000_000, output: 0 });
		run.apply(cost(0.25));
		run.apply(StreamEvent::ToolCallStart {
			id: "call-1".to_string(),
			name: "lookup".to_string(),
			args_json: serde_json::json!("{}"),
		});
		let call = run.take_pending_call("call-1".to_string(), serde_json::json!({})).ok_or("missing call")?;
		run.record_tool_result(call, Instant::now(), tool_output());
		assert!(matches!(run.finish_iteration().await, Finish::Continue));
		run.next_request().map_err(|_| "request refused")?;
		run.apply(StreamEvent::Tokens { input: 1_000_000, output: 0 });
		done_message(run.finish_iteration().await)?;
		assert_eq!(usage_cost(&pool, fixture.user_id).await?, (1, rust_decimal::Decimal::from(4)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_loop_stops_after_the_maximum_number_of_iterations(pool: PgPool) -> TestResult {
		let mut run = run(&fixture(&pool).await?);
		for _ in 0..10 {
			assert!(run.next_request().is_ok());
		}
		assert!(matches!(run.next_request(), Err(StreamData::Error { code, .. }) if code == "max_iterations"));
		Ok(())
	}
}

#[cfg(test)]
mod prepare {
	use crate::tests::gateway::support::{TestResult, create_provider_model, create_team, create_user, job_state};
	use crate::types::chat::runner::prepare_turn;
	use crate::types::*;
	use sqlx::PgPool;

	fn request(model_key: &str, regenerate: Option<&str>) -> TestResult<StreamRequest> {
		Ok(serde_json::from_value(serde_json::json!({
			"content": "hello",
			"model_key": model_key,
			"regenerate_from_message_id": regenerate,
		}))?)
	}

	struct Setup {
		state: std::sync::Arc<JobState>,
		user: User,
		chat_id: uuid::Uuid,
		}

	async fn setup(pool: &PgPool) -> TestResult<Setup> {
		let user_id = create_user(pool).await?;
		create_team(pool, user_id, "Prepare Team", true).await?;
		let (_, model_id) = create_provider_model(pool, "Prepare Provider", "prepare-model").await?;
		sqlx::query("INSERT INTO model_pricing_overrides (model_id, pricing) VALUES ($1, $2)")
			.bind(model_id)
			.bind(serde_json::json!({ "input": 1_000_000.0, "output": 1_000_000.0 }))
			.execute(pool)
			.await?;
		let user = User::find_by_id(pool, &user_id).await?.ok_or("missing user")?;
		let chat = Chat::create(pool, &user_id, None, Some("Prepare")).await?;
		Ok(Setup {
			state: job_state(pool.clone()),
			user,
			chat_id: chat.id,
		})
	}

	async fn rejection(setup: &Setup, req: &StreamRequest) -> TestResult<String> {
		match prepare_turn(&setup.state, &setup.user, setup.chat_id, req).await {
			Err(error) => Ok(format!("{error:?}")),
			Ok(_) => Err("turn was prepared".into()),
		}
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn an_exhausted_budget_rejects_the_turn_before_anything_is_stored(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let budget: uuid::Uuid = sqlx::query_scalar("INSERT INTO budgets (name, amount, kind, interval, on_exceed) VALUES ('Zero', 0, 'pooled', 'total', 'block') RETURNING id")
			.fetch_one(&pool)
			.await?;
		sqlx::query("INSERT INTO budget_assignments (budget_id, user_id) VALUES ($1, $2)")
			.bind(budget)
			.bind(setup.user.id)
			.execute(&pool)
			.await?;
		let req = request("prepare-model", None)?;
		let error = rejection(&setup, &req).await?;
		assert!(error.contains("budget_exceeded"), "{error}");
		let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE chat_id = $1").bind(setup.chat_id).fetch_one(&pool).await?;
		assert_eq!(stored, 0);
		let reservations = setup.state.budget_ledger.state_counts().await?.reservations;
		assert_eq!(reservations, 0);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn unknown_chats_and_models_are_rejected(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let req = request("missing-model", None)?;
		let error = rejection(&setup, &req).await?;
		assert!(error.contains("not_found"), "{error}");
		let other = Setup {
			chat_id: uuid::Uuid::new_v4(),
			..setup
		};
		let req = request("prepare-model", None)?;
		let error = rejection(&other, &req).await?;
		assert!(error.contains("not_found"), "{error}");
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn regenerating_needs_an_existing_message_of_the_chat(pool: PgPool) -> TestResult {
		let setup = setup(&pool).await?;
		let req = request("prepare-model", Some("not-a-uuid"))?;
		let error = rejection(&setup, &req).await?;
		assert!(error.contains("invalid_request"), "{error}");
		let missing = uuid::Uuid::new_v4().to_string();
		let req = request("prepare-model", Some(&missing))?;
		let error = rejection(&setup, &req).await?;
		assert!(error.contains("Original message not found"), "{error}");
		Ok(())
	}
}
