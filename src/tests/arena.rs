#[cfg(test)]
mod validation {
	use crate::tests::presets::validation::request;
	use crate::types::*;
	use uuid::Uuid;

	fn level(key: &str, model_id: Uuid) -> PresetLevel {
		PresetLevel {
			key: key.to_string(),
			label: None,
			hint: None,
			model_id,
			reasoning: PresetReasoning::default(),
		}
	}

	fn arena(models: &[Uuid]) -> SavePresetRequest {
		SavePresetRequest {
			scope: Some(PresetScope::Installation),
			mode: Some(PresetMode::Arena),
			levels: models.iter().enumerate().map(|(index, model)| level(&format!("m{}", index + 1), *model)).collect(),
			..request("arena")
		}
	}

	#[test]
	fn an_arena_needs_two_to_five_different_models() {
		let models: Vec<Uuid> = (0..6).map(|_| Uuid::new_v4()).collect();
		assert!(arena(&models[..2]).normalized().is_ok());
		assert!(arena(&models[..5]).normalized().is_ok());
		assert!(matches!(arena(&models[..1]).normalized(), Err(PresetError::Invalid)));
		assert!(matches!(arena(&models).normalized(), Err(PresetError::Invalid)));
		assert!(matches!(arena(&[]).normalized(), Err(PresetError::ArenaInvalid)));
		assert!(matches!(arena(&[models[0], models[0]]).normalized(), Err(PresetError::ArenaInvalid)));
	}

	#[test]
	fn an_arena_has_no_tools_and_a_shared_scope() {
		let models: Vec<Uuid> = (0..2).map(|_| Uuid::new_v4()).collect();
		let with_tools = SavePresetRequest { tool_ids: vec![Uuid::new_v4()], ..arena(&models[..2]) };
		assert!(matches!(with_tools.normalized(), Err(PresetError::ArenaInvalid)));
		let personal = SavePresetRequest { scope: Some(PresetScope::Personal), ..arena(&models[..2]) };
		assert!(matches!(personal.normalized(), Err(PresetError::ArenaInvalid)));
		let unscoped = SavePresetRequest { scope: None, ..arena(&models[..2]) };
		assert!(matches!(unscoped.normalized(), Err(PresetError::ArenaInvalid)));
	}
}

#[cfg(test)]
pub(crate) mod support {
	use crate::tests::gateway::support::{create_provider_model, create_team, job_state};
	use crate::tests::presets::scopes::actor;
	use crate::tests::presets::validation::request;
	use crate::tests::support::{TestResult, create_user};
	use crate::types::chat::runner::agent::{AgentRun, Finish};
	use crate::types::chat::runner::prepare::PreparedTurn;
	use crate::types::*;
	use omniference::stream::StreamEvent;
	use sqlx::PgPool;
	use std::sync::Arc;
	use std::time::Instant;
	use uuid::Uuid;

	pub struct Fixture {
		pub state: Arc<JobState>,
		pub user_id: Uuid,
		pub team_id: Uuid,
		pub models: Vec<Uuid>,
		pub arena: Preset,
		pub chat_id: Uuid,
	}

	pub async fn fixture(pool: &PgPool) -> TestResult<Fixture> {
		let user_id = create_user(pool).await?;
		let team_id = create_team(pool, user_id, &format!("Arena Team {}", Uuid::new_v4()), true).await?;
		let mut models = Vec::new();
		for name in ["arena-a", "arena-b"] {
			let (_, model_id) = create_provider_model(pool, &format!("Arena {}", Uuid::new_v4().simple()), name).await?;
			models.push(model_id);
		}
		let input = SavePresetRequest {
			scope: Some(PresetScope::Installation),
			mode: Some(PresetMode::Arena),
			base_model_id: models[0],
			levels: models
				.iter()
				.enumerate()
				.map(|(index, model_id)| PresetLevel {
					key: format!("m{}", index + 1),
					label: None,
					hint: None,
					model_id: *model_id,
					reasoning: PresetReasoning::default(),
				})
				.collect(),
			..request("blind")
		};
		let admin = actor(pool, user_id, true).await?;
		let arena = Preset::create(pool, &admin, input).await?;
		let chat = Chat::create(pool, &user_id, None, Some("Arena")).await?;
		Ok(Fixture {
			state: job_state(pool.clone()),
			user_id,
			team_id,
			models,
			arena,
			chat_id: chat.id,
		})
	}

	/// Stores the user message and round, then saves one finished answer per model as the runner would.
	pub async fn played_round(pool: &PgPool, fixture: &Fixture) -> TestResult<(Uuid, Vec<ChatMessageResponse>)> {
		let user_message = Message::create_user_message(pool, &fixture.chat_id, "Which is better?", None, &ReasoningDetails::default()).await?;
		let round_id = ArenaRound::create(pool, &fixture.arena.id, &fixture.chat_id, &fixture.user_id, &user_message.id).await?;
		let mut answers = Vec::new();
		for (fork_index, model_id) in (1..).zip(&fixture.models) {
			let turn = PreparedTurn {
				chat_id: fixture.chat_id,
				model_id: *model_id,
				reservation: None,
				output_estimate: 100,
				user_id: fixture.user_id,
				user_message: None,
				ir: omniference::types::ChatRequestIR::default(),
				assistant_parent_id: Some(user_message.id),
				assistant_fork_index: fork_index,
				reasoning_effort: None,
				reasoning_budget_tokens: None,
				request_settings: RequestSettings::default(),
				needs_title: false,
				active: false,
				arena_round_id: Some(round_id),
				knowledge_base_ids: Vec::new(),
			};
			let mut run = AgentRun::new(Arc::clone(&fixture.state), turn, Instant::now());
			run.next_request().await.map_err(|error| format!("{error:?}"))?;
			run.apply(StreamEvent::ReasoningDelta { content: "thinking".to_string() });
			run.apply(StreamEvent::TextDelta { content: format!("answer {fork_index}") });
			match run.finish_iteration().await {
				Finish::Stop(StreamData::Done { message }) => answers.push(message),
				_ => return Err("expected a saved answer".into()),
			}
		}
		Ok((round_id, answers))
	}
}

#[cfg(test)]
mod presets {
	use super::support::fixture;
	use crate::tests::gateway::support::insert_key;
	use crate::tests::presets::scopes::actor;
	use crate::tests::support::TestResult;
	use crate::types::*;
	use serde_json::json;
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn an_arena_is_usable_only_while_every_model_is(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let viewer = actor(&pool, fixture.user_id, false).await?;
		let responses = Preset::responses_for_user(&pool, &viewer, vec![fixture.arena.clone()]).await?;
		assert_eq!(responses[0].mode, PresetMode::Arena);
		assert_eq!(responses[0].unavailable, None);

		sqlx::query("DELETE FROM models WHERE id = $1").bind(fixture.models[1]).execute(&pool).await?;
		let arena = Preset::find_visible(&pool, &fixture.user_id, &fixture.arena.id).await?.ok_or("missing arena")?;
		let responses = Preset::responses_for_user(&pool, &viewer, vec![arena]).await?;
		assert_eq!(responses[0].unavailable, Some(PresetUnavailable::BaseModelUnavailable));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn arenas_stay_out_of_the_gateway(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let (_, token) = insert_key(&pool, Some(fixture.user_id), fixture.team_id, json!(["inference:read", "inference:write"])).await?;
		let context = GatewayCredential::authenticate(&pool, &token).await?;
		let listed = GatewayModel::list_for_context(&pool, &context).await?;
		assert!(listed.iter().all(|model| !model.id.starts_with("presets/blind")));
		let resolved = GatewayModel::resolve_inference(&pool, &context, "presets/blind").await;
		assert!(matches!(resolved, Err(GatewayModelAccessError::NotFound)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn the_mode_never_changes_and_the_admin_list_filters_by_it(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let admin = actor(&pool, fixture.user_id, true).await?;
		let mut input = crate::tests::presets::validation::request("blind");
		input.mode = Some(PresetMode::Standard);
		let error = Preset::update(&pool, &admin, &fixture.arena.id, input).await;
		assert!(matches!(error, Err(PresetError::ArenaInvalid)), "an update keeps the arena mode and its rules");

		let arenas = Preset::admin_list(&pool, &AdminPresetQuery { mode: Some(PresetMode::Arena), ..AdminPresetQuery::default() }).await?;
		assert_eq!(arenas.len(), 1);
		let standard = Preset::admin_list(&pool, &AdminPresetQuery { mode: Some(PresetMode::Standard), ..AdminPresetQuery::default() }).await?;
		assert!(standard.is_empty());
		Ok(())
	}
}

#[cfg(test)]
mod rounds {
	use super::support::{fixture, played_round};
	use crate::tests::support::{TestResult, create_user};
	use crate::types::*;
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn answers_are_saved_inactive_and_stay_blind_until_picked(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let (round_id, answers) = played_round(&pool, &fixture).await?;
		assert!(answers.iter().all(|answer| answer.arena_round_id == Some(round_id)));
		let active = Message::list_active_by_chat(&pool, &fixture.chat_id).await?;
		assert_eq!(active.len(), 1, "only the user message is active while the round is open");

		let pending = ArenaRound::pending_for_chat(&pool, &fixture.chat_id).await?.ok_or("round not pending")?;
		let blind = pending.to_response(&pool).await?;
		assert!(!blind.decided);
		assert_eq!(blind.candidates.iter().map(|candidate| candidate.label.as_str()).collect::<Vec<_>>(), ["A", "B"]);
		assert!(blind.candidates.iter().all(|candidate| candidate.model_id.is_none() && candidate.model_name.is_none() && candidate.reasoning.is_none()));
		assert!(blind.candidates.iter().all(|candidate| candidate.usage.latency_ms.is_none() && candidate.usage.time_to_first_token_ms.is_none()));
		assert!(ArenaRound::is_undecided(&pool, Some(&round_id)).await?);

		let winner = &answers[1];
		let decided = ArenaRound::pick(&pool, &fixture.user_id, &fixture.chat_id, &round_id, &winner.id).await?;
		let revealed = decided.to_response(&pool).await?;
		assert!(revealed.decided);
		assert_eq!(revealed.winner_message_id, Some(winner.id));
		assert_eq!(revealed.candidates[1].model_id, Some(fixture.models[1]));
		assert_eq!(revealed.candidates[0].reasoning.as_deref(), Some("thinking"));
		assert!(ArenaRound::pending_for_chat(&pool, &fixture.chat_id).await?.is_none());

		let history: Vec<_> = Message::list_active_by_chat(&pool, &fixture.chat_id).await?.into_iter().map(|message| message.id).collect();
		assert_eq!(history.len(), 2);
		assert_eq!(history[1], winner.id, "the next turn's history holds only the winner");
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn a_round_is_picked_once_and_only_with_its_own_answers(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let (round_id, answers) = played_round(&pool, &fixture).await?;
		let stranger = create_user(&pool).await?;
		let foreign = ArenaRound::pick(&pool, &stranger, &fixture.chat_id, &round_id, &answers[0].id).await;
		assert!(matches!(foreign, Err(ArenaError::NotFound)));
		let user_message = answers[0].parent_id.ok_or("missing parent")?;
		let not_an_answer = ArenaRound::pick(&pool, &fixture.user_id, &fixture.chat_id, &round_id, &user_message).await;
		assert!(matches!(not_an_answer, Err(ArenaError::NotFound)));

		ArenaRound::pick(&pool, &fixture.user_id, &fixture.chat_id, &round_id, &answers[0].id).await?;
		let again = ArenaRound::pick(&pool, &fixture.user_id, &fixture.chat_id, &round_id, &answers[1].id).await;
		assert!(matches!(again, Err(ArenaError::Decided)));
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn results_count_rounds_and_wins_per_model(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		for winner in [1, 1, 0] {
			let (round_id, answers) = played_round(&pool, &fixture).await?;
			ArenaRound::pick(&pool, &fixture.user_id, &fixture.chat_id, &round_id, &answers[winner].id).await?;
		}
		played_round(&pool, &fixture).await?;

		let results = Preset::arena_results(&pool, &fixture.arena.id).await?;
		let rows: Vec<(uuid::Uuid, i64, i64)> = results.iter().map(|row| (row.model_id, row.rounds, row.wins)).collect();
		assert_eq!(rows, [(fixture.models[1], 3, 2), (fixture.models[0], 3, 1)]);
		assert!((results[0].win_rate - 2.0 / 3.0).abs() < 1e-9);
		Ok(())
	}
}

#[cfg(test)]
mod prepare {
	use super::support::{Fixture, fixture, played_round};
	use crate::tests::budgets::support::{assign, create_budget};
	use crate::tests::support::TestResult;
	use crate::types::chat::runner::prepare_turn;
	use crate::types::*;
	use sqlx::PgPool;

	fn request(regenerate: Option<String>) -> TestResult<StreamRequest> {
		Ok(serde_json::from_value(serde_json::json!({
			"content": "hello",
			"model_key": "presets/blind",
			"regenerate_from_message_id": regenerate,
		}))?)
	}

	async fn rejection(pool: &PgPool, fixture: &Fixture, req: &StreamRequest) -> TestResult<String> {
		crate::config::Config::init(pool).await?;
		crate::i18n::I18n::init(pool).await;
		let user = User::find_by_id(pool, &fixture.user_id).await?.ok_or("missing user")?;
		match prepare_turn(&fixture.state, &user, fixture.chat_id, req).await {
			Err(error) => Ok(format!("{error:?}")),
			Ok(_) => Err("turn was prepared".into()),
		}
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn budget_is_reserved_for_every_model_or_none(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		sqlx::query("INSERT INTO model_pricing_overrides (model_id, pricing) VALUES ($1, $2)")
			.bind(fixture.models[1])
			.bind(serde_json::json!({ "input": 1_000_000.0, "output": 1_000_000.0 }))
			.execute(&pool)
			.await?;
		let budget = create_budget(&pool, "0", "block").await?;
		assign(&pool, budget, Some(fixture.user_id), None, None).await?;

		let req = request(None)?;
		let error = rejection(&pool, &fixture, &req).await?;
		assert!(error.contains("budget_exceeded"), "{error}");
		let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE chat_id = $1").bind(fixture.chat_id).fetch_one(&pool).await?;
		assert_eq!(stored, 0);
		assert_eq!(fixture.state.budget_ledger.state_counts().await?.reservations, 0);
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn arena_answers_cannot_be_regenerated(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		let req = request(Some(uuid::Uuid::new_v4().to_string()))?;
		let error = rejection(&pool, &fixture, &req).await?;
		assert!(error.contains("invalid_request"), "{error}");
		Ok(())
	}

	#[sqlx::test(migrations = "./migrations")]
	async fn an_open_round_blocks_the_chat_until_picked(pool: PgPool) -> TestResult {
		let fixture = fixture(&pool).await?;
		played_round(&pool, &fixture).await?;
		let req = request(None)?;
		let error = rejection(&pool, &fixture, &req).await?;
		assert!(error.contains("arena_pending"), "{error}");
		Ok(())
	}
}
