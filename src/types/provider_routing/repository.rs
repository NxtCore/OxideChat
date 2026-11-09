use super::{EffectiveProviderRouting, ProviderAllowlists, TeamProviderRouting};
use crate::config::Config;
use sqlx::PgPool;
use sqlx::types::Json;
use uuid::Uuid;

type TeamRoutingRow = (Option<bool>, Json<ProviderAllowlists>);

impl EffectiveProviderRouting {
	/// The routing settings for a user's own requests, combining every team the user belongs to.
	///
	/// # Errors
	/// Returns an error when the database query fails.
	pub async fn for_user(pool: &PgPool, user_id: &Uuid) -> Result<Self, sqlx::Error> {
		let teams = sqlx::query_as::<_, TeamRoutingRow>(
			r#"
			SELECT t.provider_selector_enabled, t.allowed_upstream_providers
			FROM teams t
			JOIN team_members tm ON tm.team_id = t.id
			WHERE tm.user_id = $1
			"#,
		)
		.bind(user_id)
		.fetch_all(pool)
		.await?;
		Ok(Self::with_teams(teams))
	}

	/// The routing settings for requests made with one of a team's API keys.
	///
	/// # Errors
	/// Returns an error when the database query fails.
	pub async fn for_team(pool: &PgPool, team_id: &Uuid) -> Result<Self, sqlx::Error> {
		let teams = sqlx::query_as::<_, TeamRoutingRow>("SELECT provider_selector_enabled, allowed_upstream_providers FROM teams WHERE id = $1")
			.bind(team_id)
			.fetch_all(pool)
			.await?;
		Ok(Self::with_teams(teams))
	}

	/// The installation's settings alone, for requests that belong to no user or team.
	#[must_use]
	pub fn installation() -> Self {
		Self::combine(&Config::get().provider_routing(), &[])
	}

	fn with_teams(rows: Vec<TeamRoutingRow>) -> Self {
		let teams: Vec<TeamProviderRouting> = rows
			.into_iter()
			.map(|(selector_enabled, Json(allowed_providers))| TeamProviderRouting { selector_enabled, allowed_providers })
			.collect();
		Self::combine(&Config::get().provider_routing(), &teams)
	}
}
