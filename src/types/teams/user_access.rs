use super::{ModelGrantKind, ModelGrantTarget, Team, TeamRole, TeamSummaryResponse, UserModelAccessResponse, UserModelGrant, UserTeamResponse};
use sqlx::{PgExecutor, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct ModelGrantRow {
	team_id: Uuid,
	team_name: String,
	is_default: bool,
	kind: String,
	target_id: Option<Uuid>,
	target_name: Option<String>,
}

impl From<ModelGrantRow> for UserModelGrant {
	fn from(row: ModelGrantRow) -> Self {
		Self {
			team: TeamSummaryResponse {
				id: row.team_id,
				name: row.team_name,
				is_default: row.is_default,
			},
			kind: ModelGrantKind::from_stored(&row.kind),
			target: row.target_id.zip(row.target_name).map(|(id, name)| ModelGrantTarget { id, name }),
		}
	}
}

impl Team {
	/// The teams a user belongs to with their role in each, the default team first.
	///
	/// # Errors
	/// Returns the database error when the query fails.
	pub async fn memberships_for_user(executor: impl PgExecutor<'_>, user_id: &Uuid) -> Result<Vec<UserTeamResponse>, sqlx::Error> {
		let rows = sqlx::query_as::<_, (Uuid, String, bool, String)>(
			"SELECT t.id, t.name::text, t.is_default, tm.role::text FROM teams t JOIN team_members tm ON tm.team_id = t.id \
			 WHERE tm.user_id = $1 ORDER BY t.is_default DESC, lower(t.name)",
		)
		.bind(user_id)
		.fetch_all(executor)
		.await?;
		Ok(rows
			.into_iter()
			.map(|(id, name, is_default, role)| UserTeamResponse {
				id,
				name,
				is_default,
				role: TeamRole::from_stored(&role),
			})
			.collect())
	}

	/// Every grant that lets a user use models, and how many of the enabled models they cover.
	///
	/// Model access only comes from teams: a team allows all models, every model of a provider, or single models.
	///
	/// # Errors
	/// Returns the database error when a query fails.
	pub async fn model_access_for_user(pool: &PgPool, user_id: &Uuid) -> Result<UserModelAccessResponse, sqlx::Error> {
		let grants = sqlx::query_as::<_, ModelGrantRow>(
			r"
			SELECT t.id AS team_id, t.name::text AS team_name, t.is_default, 'all' AS kind, NULL::uuid AS target_id, NULL::text AS target_name
			FROM teams t
			JOIN team_members tm ON tm.team_id = t.id AND tm.user_id = $1
			WHERE t.allow_all_models
			UNION ALL
			SELECT t.id, t.name::text, t.is_default, 'provider', p.id, p.name::text
			FROM team_model_access a
			JOIN teams t ON t.id = a.team_id
			JOIN team_members tm ON tm.team_id = t.id AND tm.user_id = $1
			JOIN providers p ON p.id = a.provider_id
			UNION ALL
			SELECT t.id, t.name::text, t.is_default, 'model', m.id, m.display_name::text
			FROM team_model_access a
			JOIN teams t ON t.id = a.team_id
			JOIN team_members tm ON tm.team_id = t.id AND tm.user_id = $1
			JOIN models m ON m.id = a.model_id
			ORDER BY is_default DESC, team_name, kind, target_name
			",
		)
		.bind(user_id)
		.fetch_all(pool)
		.await?;
		let (usable, total): (i64, i64) = sqlx::query_as(
			r"
			SELECT COUNT(*) FILTER (WHERE EXISTS (
				SELECT 1
				FROM team_members tm
				JOIN teams t ON t.id = tm.team_id
				WHERE tm.user_id = $1
				  AND (t.allow_all_models OR EXISTS (
					SELECT 1 FROM team_model_access a WHERE a.team_id = t.id AND (a.model_id = m.id OR a.provider_id = p.id)
				  ))
			)), COUNT(*)
			FROM models m
			JOIN providers p ON p.id = m.provider_id
			WHERE COALESCE(m.is_enabled, false) AND COALESCE(p.is_enabled, false)
			",
		)
		.bind(user_id)
		.fetch_one(pool)
		.await?;
		Ok(UserModelAccessResponse {
			usable,
			total,
			grants: grants.into_iter().map(UserModelGrant::from).collect(),
		})
	}

	/// Makes the default team and `team_ids` the only teams of a user inside the caller's transaction; memberships that stay keep their role.
	///
	/// Returns the teams the user left and the teams they joined.
	///
	/// # Errors
	/// Returns the database error when a team does not exist or a statement fails.
	pub async fn replace_user_teams(tx: &mut Transaction<'_, Postgres>, user_id: &Uuid, team_ids: &[Uuid]) -> Result<(Vec<Uuid>, Vec<Uuid>), sqlx::Error> {
		let left = sqlx::query_scalar(
			"DELETE FROM team_members tm USING teams t \
			 WHERE t.id = tm.team_id AND tm.user_id = $1 AND NOT t.is_default AND tm.team_id <> ALL($2) RETURNING tm.team_id",
		)
		.bind(user_id)
		.bind(team_ids)
		.fetch_all(&mut **tx)
		.await?;
		let joined = sqlx::query_scalar(
			"INSERT INTO team_members (team_id, user_id) \
			 SELECT id, $1::uuid FROM teams WHERE is_default UNION SELECT team_id, $1::uuid FROM unnest($2::uuid[]) AS team_id \
			 ON CONFLICT DO NOTHING RETURNING team_id",
		)
		.bind(user_id)
		.bind(team_ids)
		.fetch_all(&mut **tx)
		.await?;
		Ok((left, joined))
	}
}
