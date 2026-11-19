use super::{Preset, PresetArena};
use crate::types::UsageDetails;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

/// `oxide_request_type` of the requests an arena round sends.
pub const ARENA_REQUEST_TYPE: &str = "arena";

/// Why an arena round cannot be picked or looked at.
#[derive(Debug, thiserror::Error)]
pub enum ArenaError {
	#[error("the arena round or answer does not exist")]
	NotFound,
	#[error("the arena round is already decided")]
	Decided,
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}

/// A row of `arena_rounds`: one blind comparison in a chat.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ArenaRound {
	pub id: Uuid,
	pub preset_id: Uuid,
	pub user_message_id: Uuid,
	pub winner_message_id: Option<Uuid>,
	pub decided_at: Option<DateTime<Utc>>,
}

/// One answer of a round; its model is only named once the round is decided.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ArenaCandidateResponse {
	pub message_id: Uuid,
	pub label: String,
	pub content: String,
	pub reasoning: Option<String>,
	#[cfg_attr(test, ts(type = "Record<string, number | string | null>"))]
	pub usage: UsageDetails,
	pub model_id: Option<Uuid>,
	pub model_key: Option<String>,
	pub model_name: Option<String>,
}

/// A round with its answers as the user sees it: blind until decided.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ArenaRoundResponse {
	pub id: Uuid,
	pub user_message_id: Uuid,
	pub decided: bool,
	pub winner_message_id: Option<Uuid>,
	pub stream_responses: bool,
	pub show_reasoning: bool,
	pub candidates: Vec<ArenaCandidateResponse>,
}

/// Body of a pick.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ArenaPickRequest {
	pub message_id: Uuid,
}

/// How often one model of an arena was compared and picked.
#[derive(Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ArenaResultRow {
	pub model_id: Uuid,
	pub model_name: String,
	pub rounds: i64,
	pub wins: i64,
	pub win_rate: f64,
}

#[derive(sqlx::FromRow)]
struct CandidateRow {
	id: Uuid,
	content: String,
	reasoning_content: Option<String>,
	usage_details: Json<UsageDetails>,
	model_id: Option<Uuid>,
	model_key: Option<String>,
	model_name: Option<String>,
}

macro_rules! round_columns {
	() => {
		"r.id, r.preset_id, r.user_message_id, r.winner_message_id, r.decided_at"
	};
}

/// Removes what would give the model away: timings, and reasoning unless the arena shows it.
pub fn redact_usage(usage: &mut UsageDetails) {
	usage.latency_ms = None;
	usage.reasoning_latency_ms = None;
	usage.time_to_first_token_ms = None;
	usage.generation_ms = None;
}

impl ArenaRound {
	/// Starts a round for the user message just stored.
	///
	/// # Errors
	/// Returns the database error when the insert fails.
	pub async fn create(pool: &PgPool, preset_id: &Uuid, chat_id: &Uuid, user_id: &Uuid, user_message_id: &Uuid) -> Result<Uuid, sqlx::Error> {
		sqlx::query_scalar("INSERT INTO arena_rounds (preset_id, chat_id, user_id, user_message_id) VALUES ($1, $2, $3, $4) RETURNING id")
			.bind(preset_id)
			.bind(chat_id)
			.bind(user_id)
			.bind(user_message_id)
			.fetch_one(pool)
			.await
	}

	/// A round of a chat the user owns.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn find_for_user(pool: &PgPool, user_id: &Uuid, chat_id: &Uuid, round_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!(
			"SELECT ",
			round_columns!(),
			" FROM arena_rounds r JOIN chats c ON c.id = r.chat_id WHERE r.id = $1 AND r.chat_id = $2 AND c.user_id = $3"
		))
		.bind(round_id)
		.bind(chat_id)
		.bind(user_id)
		.fetch_optional(pool)
		.await
	}

	/// The newest undecided round of a chat that has at least one saved answer; it blocks the chat until one is picked.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn pending_for_chat(pool: &PgPool, chat_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(concat!(
			"SELECT ",
			round_columns!(),
			" FROM arena_rounds r JOIN messages u ON u.id = r.user_message_id AND u.is_active_fork \
			 WHERE r.chat_id = $1 AND r.decided_at IS NULL AND EXISTS (SELECT 1 FROM messages m WHERE m.arena_round_id = r.id) \
			 ORDER BY r.created_at DESC LIMIT 1"
		))
		.bind(chat_id)
		.fetch_optional(pool)
		.await
	}

	/// Whether `round_id` names a round that is still undecided, so its answers must stay blind.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn is_undecided(pool: &PgPool, round_id: Option<&Uuid>) -> Result<bool, sqlx::Error> {
		let Some(round_id) = round_id else { return Ok(false) };
		sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM arena_rounds WHERE id = $1 AND decided_at IS NULL)")
			.bind(round_id)
			.fetch_one(pool)
			.await
	}

	/// The round with its answers labelled A, B, … in the order they were shown; models, timings and hidden reasoning
	/// are left out until the round is decided.
	///
	/// # Errors
	/// Returns the database error when a query fails.
	pub async fn to_response(&self, pool: &PgPool) -> Result<ArenaRoundResponse, sqlx::Error> {
		let options: PresetArena = sqlx::query_scalar::<_, Json<PresetArena>>("SELECT arena FROM model_presets WHERE id = $1")
			.bind(self.preset_id)
			.fetch_optional(pool)
			.await?
			.map(|options| options.0)
			.unwrap_or_default();
		let rows = sqlx::query_as::<_, CandidateRow>(
			"SELECT m.id, m.content, m.reasoning_content, m.usage_details, m.model_id, m.request_settings->>'model_key' AS model_key, mo.display_name AS model_name \
			 FROM messages m LEFT JOIN models mo ON mo.id = m.model_id WHERE m.arena_round_id = $1 ORDER BY m.fork_index",
		)
		.bind(self.id)
		.fetch_all(pool)
		.await?;
		let decided = self.decided_at.is_some();
		let candidates = rows
			.into_iter()
			.zip('A'..='Z')
			.map(|(row, label)| {
				let mut usage = row.usage_details.0;
				if !decided {
					redact_usage(&mut usage);
				}
				ArenaCandidateResponse {
					message_id: row.id,
					label: label.to_string(),
					content: row.content,
					reasoning: row.reasoning_content.filter(|_| decided || options.show_reasoning),
					usage,
					model_id: row.model_id.filter(|_| decided),
					model_key: row.model_key.filter(|_| decided),
					model_name: row.model_name.filter(|_| decided),
				}
			})
			.collect();
		Ok(ArenaRoundResponse {
			id: self.id,
			user_message_id: self.user_message_id,
			decided,
			winner_message_id: self.winner_message_id,
			stream_responses: options.stream_responses,
			show_reasoning: options.show_reasoning,
			candidates,
		})
	}

	/// Picks the winning answer of an undecided round of the user's chat and makes it the chat's active reply; the other
	/// answers stay as inactive siblings.
	///
	/// # Errors
	/// Returns [`ArenaError::NotFound`] when the round or the answer is not the user's, and [`ArenaError::Decided`] when
	/// the round was already decided.
	pub async fn pick(pool: &PgPool, user_id: &Uuid, chat_id: &Uuid, round_id: &Uuid, message_id: &Uuid) -> Result<Self, ArenaError> {
		let mut tx = pool.begin().await?;
		let round = sqlx::query_as::<_, Self>(concat!(
			"SELECT ",
			round_columns!(),
			" FROM arena_rounds r JOIN chats c ON c.id = r.chat_id WHERE r.id = $1 AND r.chat_id = $2 AND c.user_id = $3 FOR UPDATE OF r"
		))
		.bind(round_id)
		.bind(chat_id)
		.bind(user_id)
		.fetch_optional(&mut *tx)
		.await?
		.ok_or(ArenaError::NotFound)?;
		if round.decided_at.is_some() {
			return Err(ArenaError::Decided);
		}
		let winner_model: Option<Option<Uuid>> = sqlx::query_scalar("UPDATE messages SET is_active_fork = TRUE WHERE id = $1 AND arena_round_id = $2 RETURNING model_id")
			.bind(message_id)
			.bind(round_id)
			.fetch_optional(&mut *tx)
			.await?;
		let winner_model = winner_model.ok_or(ArenaError::NotFound)?;
		let round = sqlx::query_as::<_, Self>(
			"UPDATE arena_rounds r SET winner_message_id = $2, winner_model_id = $3, decided_at = NOW() WHERE id = $1 \
			 RETURNING r.id, r.preset_id, r.user_message_id, r.winner_message_id, r.decided_at",
		)
		.bind(round_id)
		.bind(message_id)
		.bind(winner_model)
		.fetch_one(&mut *tx)
		.await?;
		touch(&mut *tx, chat_id).await?;
		tx.commit().await?;
		Ok(round)
	}
}

async fn touch(executor: impl PgExecutor<'_>, chat_id: &Uuid) -> Result<(), sqlx::Error> {
	sqlx::query("UPDATE chats SET updated_at = NOW() WHERE id = $1").bind(chat_id).execute(executor).await?;
	Ok(())
}

impl Preset {
	/// For each model of the arena: the decided rounds it answered in and how often it was picked, most wins first.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn arena_results(pool: &PgPool, id: &Uuid) -> Result<Vec<ArenaResultRow>, sqlx::Error> {
		sqlx::query_as::<_, ArenaResultRow>(
			"SELECT m.model_id, mo.display_name AS model_name, COUNT(DISTINCT r.id) AS rounds, \
			        COUNT(DISTINCT r.id) FILTER (WHERE r.winner_model_id = m.model_id) AS wins, \
			        (COUNT(DISTINCT r.id) FILTER (WHERE r.winner_model_id = m.model_id))::float8 / COUNT(DISTINCT r.id) AS win_rate \
			 FROM arena_rounds r JOIN messages m ON m.arena_round_id = r.id JOIN models mo ON mo.id = m.model_id \
			 WHERE r.preset_id = $1 AND r.decided_at IS NOT NULL \
			 GROUP BY m.model_id, mo.display_name ORDER BY wins DESC, lower(mo.display_name)",
		)
		.bind(id)
		.fetch_all(pool)
		.await
	}
}
