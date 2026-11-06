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
	use crate::types::chat::runner::agent::{AgentRun, Finish, StepError, ToolOutput};
	use crate::types::chat::runner::prepare::PreparedTurn;
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
	}

	async fn fixture(pool: &PgPool) -> TestResult<Fixture> {
		let user_id = create_user(pool).await?;
		create_team(pool, user_id, "Chat Team", true).await?;
		let (_, model_id) = create_provider_model(pool, "Chat Provider", "chat-model").await?;
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
		})
	}

	fn run(fixture: &Fixture) -> AgentRun {
		let mut ir = omniference::types::ChatRequestIR::default();
		ir.metadata.insert("oxide_user_id".to_string(), fixture.user_id.to_string());
		let turn = PreparedTurn {
			chat_id: fixture.chat_id,
			model_id: fixture.model_id,
			reservation: None,
			output_estimate: 1_000,
			user_id: fixture.user_id,
			user_message: None,
			ir,
			assistant_parent_id: None,
			assistant_fork_index: 1,
			reasoning_effort: None,
			reasoning_budget_tokens: None,
			request_settings: RequestSettings::default(),
			needs_title: false,
		};
		AgentRun::new(Arc::clone(&fixture.state), turn, Instant::now())
	}

	fn cost(total: f64) -> StreamEvent {
		StreamEvent::Cost {
			cost: omniference::stream::CostDetails {
				total,
				prompt: Some(total / 2.0),
				completion: Some(total / 2.0),
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

	fn tool_step(run: &mut AgentRun, call_id: &str) -> TestResult {
		run.apply(StreamEvent::ToolCallStart {
			id: call_id.to_string(),
			name: "lookup".to_string(),
			args_json: serde_json::json!("{}"),
		});
		let call = run.take_pending_call(call_id.to_string(), serde_json::json!({})).ok_or("missing call")?;
		run.record_tool_result(call, Instant::now(), tool_output());
		Ok(())
	}

	async fn usage_events(pool: &PgPool, user_id: Uuid) -> TestResult<i64> {
		Ok(sqlx::query_scalar("SELECT COUNT(*) FROM usage_events WHERE user_id = $1").bind(user_id).fetch_one(pool).await?)
	}

	async fn exhaust_budget(pool: &PgPool, user_id: Uuid) -> TestResult {
		let budget: Uuid = sqlx::query_scalar("INSERT INTO budgets (name, amount, kind, interval, on_exceed) VALUES ('Zero', 0, 'pooled', 'total', 'block') RETURNING id")
			.fetch_one(pool)
			.await?;
		sqlx::query("INSERT INTO budget_assignments (budget_id, user_id) VALUES ($1, $2)")
			.bind(budget)
			.bind(user_id)
			.execute(pool)
			.await?;
		Ok(())
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
	async fn a_turn_saves_its_message_and_leaves_usage_to_the_cost_sink(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let mut run = run(&fixture);
		run.next_request().await.map_err(|_| "request refused")?;
		run.apply(StreamEvent::TextDelta { content: "Hello ".to_string() });
		run.apply(StreamEvent::TextDelta { content: "world".to_string() });
		run.apply(StreamEvent::Tokens { input: 1_000_000, output: 0 });
		run.apply(cost(2.0));
		let message = done_message(run.finish_iteration().await)?;
		assert_eq!(message.content, "Hello world");
		assert_eq!(message.cost_details.total, Some(2.0));
		assert_eq!(usage_events(&pool, fixture.user_id).await?, 0);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_cost_of_every_step_adds_up_on_the_message(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let mut run = run(&fixture);
		run.next_request().await.map_err(|_| "request refused")?;
		run.apply(StreamEvent::Tokens { input: 1_000, output: 10 });
		run.apply(cost(0.25));
		tool_step(&mut run, "call-1")?;
		assert!(matches!(run.finish_iteration().await, Finish::Continue));

		let request = run.next_request().await.map_err(|_| "request refused")?;
		assert_eq!(request.messages.len(), 2);
		run.apply(StreamEvent::TextDelta { content: "Done".to_string() });
		run.apply(StreamEvent::Tokens { input: 2_000, output: 20 });
		run.apply(cost(0.5));
		let message = done_message(run.finish_iteration().await)?;
		assert_eq!(message.content, "Done");
		assert!(message.content_parts.ok_or("missing parts")?.to_string().contains("tool_call"));
		assert_eq!(message.cost_details.total, Some(0.75));
		assert_eq!(message.cost_details.input, Some(0.375));
		assert_eq!(message.usage_details.input_tokens, Some(3_000));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn every_step_after_the_first_reserves_its_own_budget(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let mut run = run(&fixture);
		let first = run.next_request().await.map_err(|_| "request refused")?;
		assert_eq!(first.metadata.get("oxide_user_id"), Some(&fixture.user_id.to_string()));
		assert!(!first.metadata.contains_key(BUDGET_RESERVATION_METADATA_KEY));
		tool_step(&mut run, "call-1")?;
		assert!(matches!(run.finish_iteration().await, Finish::Continue));

		let second = run.next_request().await.map_err(|_| "request refused")?;
		let reserved = second.metadata.get(BUDGET_RESERVATION_METADATA_KEY).ok_or("second step has no reservation")?;
		tool_step(&mut run, "call-2")?;
		assert!(matches!(run.finish_iteration().await, Finish::Continue));
		let third = run.next_request().await.map_err(|_| "request refused")?;
		assert_ne!(third.metadata.get(BUDGET_RESERVATION_METADATA_KEY), Some(reserved));
		assert_eq!(fixture.state.budget_ledger.state_counts().await?.reservations, 1);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_step_refused_by_the_budget_keeps_the_answer_so_far(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let mut run = run(&fixture);
		run.next_request().await.map_err(|_| "request refused")?;
		run.apply(StreamEvent::TextDelta { content: "Partial".to_string() });
		run.apply(cost(0.25));
		tool_step(&mut run, "call-1")?;
		assert!(matches!(run.finish_iteration().await, Finish::Continue));
		exhaust_budget(&pool, fixture.user_id).await?;

		let refused = run.next_request().await.err().ok_or("the step was not refused")?;
		assert_eq!(refused.code, "budget_exceeded");
		let saved = run.save_partial().await.ok_or("nothing was saved")?;
		let StreamData::Done { message } = saved else {
			return Err("expected the saved message".into());
		};
		assert_eq!(message.content, "Partial");
		assert_eq!(message.cost_details.total, Some(0.25));
		let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE chat_id = $1").bind(fixture.chat_id).fetch_one(&pool).await?;
		assert_eq!(stored, 1);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_turn_that_produced_nothing_saves_nothing(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let mut run = run(&fixture);
		run.next_request().await.map_err(|_| "request refused")?;
		assert!(run.save_partial().await.is_none());
		let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE chat_id = $1").bind(fixture.chat_id).fetch_one(&pool).await?;
		assert_eq!(stored, 0);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_loop_stops_after_the_maximum_number_of_iterations(pool: PgPool) -> TestResult {
		let mut run = run(&fixture(&pool).await?);
		for _ in 0..10 {
			assert!(run.next_request().await.is_ok());
		}
		assert!(matches!(run.next_request().await, Err(StepError { code: "max_iterations", .. })));
		Ok(())
	}
}

#[cfg(test)]
mod accounting {
	use crate::tests::gateway::support::{TestResult, create_provider_model, create_team, create_user, job_state};
	use crate::types::*;
	use crate::utils::omniference_cost::OxideCostQueue;
	use omniference::catalog::UsageBreakdown;
	use omniference::middleware::cost::{CostFinalization, CostRecord, CostSink};
	use omniference::stream::CostDetails;
	use rust_decimal::Decimal;
	use sqlx::PgPool;
	use std::collections::BTreeMap;
	use uuid::Uuid;

	#[sqlx::test(migrations = "./migrations")]
	async fn each_chat_request_becomes_a_usage_event_without_a_request_log(pool: PgPool) -> TestResult {
		let user_id = create_user(&pool).await?;
		let team_id = create_team(&pool, user_id, "Chat Team", true).await?;
		let (_, model_id) = create_provider_model(&pool, "Chat Provider", "chat-model").await?;
		let state = job_state(pool.clone());
		let admission = state.budget_ledger.admit_checks(Vec::new(), Decimal::from(5)).await?;
		let reservation = admission.reservation.ok_or("no reservation")?;
		let metadata = BTreeMap::from([
			("oxide_user_id".to_string(), user_id.to_string()),
			("oxide_model_id".to_string(), model_id.to_string()),
			("oxide_request_type".to_string(), "chat".to_string()),
			(BUDGET_RESERVATION_METADATA_KEY.to_string(), reservation.id.to_string()),
		]);
		let queue = OxideCostQueue::spawn(std::sync::Arc::clone(&state));
		for total in [0.25, 0.5] {
			queue.record_with_context(&CostRecord {
				provider: "Chat Provider".to_owned(),
				model: "chat-model".to_owned(),
				cost: CostDetails {
					total,
					prompt: None,
					completion: None,
					reasoning: None,
				},
				finalization: CostFinalization::Done,
				metadata: metadata.clone(),
				usage: UsageBreakdown {
					input_tokens: 3,
					output_tokens: 4,
					..Default::default()
				},
				finish_reason: None,
				time_to_first_token: None,
			});
		}
		queue.shutdown().await;

		let rows: Vec<(String, Option<Uuid>, Decimal)> = sqlx::query_as("SELECT request_type, team_id, cost_total FROM usage_events WHERE user_id = $1 ORDER BY cost_total")
			.bind(user_id)
			.fetch_all(&pool)
			.await?;
		assert_eq!(rows, vec![("chat".to_string(), Some(team_id), "0.25".parse()?), ("chat".to_string(), Some(team_id), "0.5".parse()?)]);
		let logs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gateway_request_logs").fetch_one(&pool).await?;
		assert_eq!(logs, 0);
		drop(reservation);
		assert_eq!(state.budget_ledger.state_counts().await?.reservations, 0);
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

#[cfg(test)]
mod title {
	use crate::types::chat::runner::title::clean_title;

	#[test]
	fn keeps_the_first_line_without_quotes_or_markdown() {
		assert_eq!(clean_title("\n  \"Rust lifetimes explained.\"\nmore"), Some("Rust lifetimes explained".to_string()));
		assert_eq!(clean_title("## **Trip to Lisbon**"), Some("Trip to Lisbon".to_string()));
	}

	#[test]
	fn empty_replies_give_no_title_and_long_ones_are_cut() {
		assert_eq!(clean_title("  \n \"\" "), None);
		assert_eq!(clean_title(&"a".repeat(200)).map(|title| title.chars().count()), Some(80));
	}
}
