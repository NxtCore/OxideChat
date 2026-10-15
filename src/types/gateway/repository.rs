use super::{GatewayAuthContext, GatewayAuthError, GatewayCredential, GatewayInference, GatewayModel, GatewayModelAccessError};
use super::{GatewayRatePolicy, GatewayRateRule};
use crate::types::{BudgetAdmission, BudgetLedger, TokenEstimate};
use crate::utils::auth::{hash_password, verify_password};
use chrono::Utc;
use omniference::types::providers::OpenAIModel;
use sqlx::{PgPool, types::Json};
use std::sync::{Arc, LazyLock};
use uuid::Uuid;

// Internal Types

struct ResolvedModel {
	id: Uuid,
	engine_model_id: String,
	max_tokens: Option<i32>,
	context_length: Option<i32>,
}

impl From<ResolvedModel> for GatewayInference {
	fn from(model: ResolvedModel) -> Self {
		Self {
			model_id: model.id,
			engine_model_id: model.engine_model_id,
			max_output_tokens: model.max_tokens.and_then(|value| u32::try_from(value).ok()).filter(|value| *value > 0),
			context_length: model.context_length.and_then(|value| u32::try_from(value).ok()).filter(|value| *value > 0),
			budget: BudgetAdmission::default(),
		}
	}
}

static DUMMY_SECRET_HASH: LazyLock<String> = LazyLock::new(|| hash_password("oxide-gateway-invalid-secret").unwrap_or_default());

impl GatewayCredential {
	/// Authenticates a gateway token and returns its enabled team-pinned context.
	///
	/// Invalid tokens and disabled, revoked, or expired keys return
	/// [`GatewayAuthError::Invalid`]. Database and verification-task failures
	/// return [`GatewayAuthError::Unavailable`].
	///
	/// # Errors
	///
	/// Returns a gateway authentication error when the credential is invalid or
	/// the authentication service is unavailable.
	pub async fn authenticate(pool: &PgPool, token: &str) -> Result<GatewayAuthContext, GatewayAuthError> {
		let parsed = parse_token(token);
		let credential = match parsed.as_ref() {
			Some(parsed) => Self::find(pool, &parsed.key_id).await.map_err(|error| {
				tracing::error!(%error, key_id = %parsed.key_id, "gateway credential or policy could not be loaded");
				GatewayAuthError::Unavailable
			})?,
			None => None,
		};
		let hash = credential.as_ref().map_or(DUMMY_SECRET_HASH.as_str(), |value| value.secret_hash.as_str());
		let secret = parsed.map_or_else(String::new, |parsed| parsed.secret.to_owned());
		let hash = hash.to_owned();
		let verified = tokio::task::spawn_blocking(move || verify_password(&secret, &hash).unwrap_or(false))
			.await
			.map_err(|_| GatewayAuthError::Unavailable)?;
		let Some(credential) = credential else {
			return Err(GatewayAuthError::Invalid);
		};
		if !verified
			|| !credential.key_enabled
			|| credential.revoked_at.is_some()
			|| credential.expires_at.is_some_and(|expires_at| expires_at <= Utc::now())
		{
			return Err(GatewayAuthError::Invalid);
		}
		if let Err(error) = sqlx::query!(
			"UPDATE gateway_api_keys SET last_used_at = NOW() WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < NOW() - INTERVAL '1 minute')",
			credential.key_id
		)
		.execute(pool)
		.await
		{
			tracing::warn!(%error, key_id = %credential.key_id, "failed to update gateway API key usage timestamp");
		}
		let policy = GatewayRatePolicy::from_stored(
			credential.team_rules.0,
			credential.key_rules.0,
			credential.team_concurrency,
			credential.key_concurrency,
		)
		.map_err(|error| {
			tracing::error!(%error, team_id = %credential.team_id, key_id = %credential.key_id, "invalid gateway rate policy");
			GatewayAuthError::Unavailable
		})?;
		Ok(GatewayAuthContext {
			key_id: credential.key_id,
			user_id: credential.user_id,
			team_id: credential.team_id,
			scopes: credential.scopes.0,
			policy: Arc::new(policy),
		})
	}

	async fn find(pool: &PgPool, key_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as!(
			Self,
			r#"
			SELECT
				k.id AS "key_id!",
				k.user_id AS "user_id!",
				k.team_id,
				k.secret_hash,
				k.scopes AS "scopes!: Json<Vec<String>>",
				k.is_enabled AS "key_enabled!",
				k.expires_at,
				k.revoked_at,
				t.gateway_max_concurrent_requests AS team_concurrency,
				k.max_concurrent_requests AS key_concurrency,
				rules.team_rules AS "team_rules!: Json<Vec<GatewayRateRule>>",
				rules.key_rules AS "key_rules!: Json<Vec<GatewayRateRule>>"
			FROM gateway_api_keys k
			JOIN teams t ON t.id = k.team_id
			JOIN team_members tm ON tm.team_id = k.team_id AND tm.user_id = k.user_id
			CROSS JOIN LATERAL (
				SELECT
					COALESCE(jsonb_agg(r.rule ORDER BY r.id) FILTER (WHERE r.team_id = t.id), '[]'::jsonb) AS team_rules,
					COALESCE(jsonb_agg(r.rule ORDER BY r.id) FILTER (WHERE r.api_key_id = k.id), '[]'::jsonb) AS key_rules
				FROM (
					SELECT id, team_id, api_key_id, jsonb_build_object('id', id, 'route_class', route_class, 'metric', metric, 'capacity', capacity, 'refill_period_seconds', refill_period_seconds) AS rule
					FROM gateway_rate_limit_rules
					WHERE is_enabled AND (team_id = t.id OR api_key_id = k.id)
				) r
			) rules
			WHERE k.id = $1
			"#,
			key_id
		)
		.fetch_optional(pool)
		.await
	}
}

impl GatewayModel {
	/// Lists enabled models accessible through the key's pinned team.
	///
	/// Only the key's pinned team is considered.
	///
	/// # Errors
	///
	/// Returns an error when model-access storage cannot be queried.
	pub async fn list_for_context(pool: &PgPool, context: &GatewayAuthContext) -> Result<Vec<OpenAIModel>, sqlx::Error> {
		let models = sqlx::query_as!(
			Self,
			r#"
			SELECT DISTINCT
				p.name AS "provider_name!",
				m.model_id,
				m.created_at
			FROM models m
			JOIN providers p ON p.id = m.provider_id
			WHERE m.is_enabled = TRUE
			  AND p.is_enabled = TRUE
			  AND EXISTS (
				SELECT 1
				FROM team_members tm
				JOIN teams t ON t.id = tm.team_id
				LEFT JOIN team_model_access model_access
					ON model_access.team_id = t.id AND model_access.model_id = m.id
				LEFT JOIN team_model_access provider_access
					ON provider_access.team_id = t.id AND provider_access.provider_id = p.id
				WHERE tm.user_id = $1
				  AND t.id = $2
				  AND (t.allow_all_models OR model_access.id IS NOT NULL OR provider_access.id IS NOT NULL)
			  )
			ORDER BY p.name, m.model_id
			"#,
			context.user_id,
			context.team_id
		)
		.fetch_all(pool)
		.await?;
		Ok(models.into_iter().map(OpenAIModel::from).collect())
	}

	/// Resolves one enabled provider/model identifier accessible to the context.
	///
	/// Provider-qualified identifiers match provider names case-insensitively.
	/// An unqualified identifier resolves only when exactly one distinct model is
	/// accessible; otherwise this returns `None`.
	///
	/// # Errors
	///
	/// Returns an error when model-access storage cannot be queried.
	pub async fn resolve_accessible(pool: &PgPool, context: &GatewayAuthContext, requested_id: &str) -> Result<Option<Uuid>, sqlx::Error> {
		Ok(Self::resolve_model(pool, context, requested_id).await?.map(|model| model.id))
	}

	async fn resolve_model(pool: &PgPool, context: &GatewayAuthContext, requested_id: &str) -> Result<Option<ResolvedModel>, sqlx::Error> {
		let (provider_name, model_id) = requested_id.split_once('/').map_or((None, requested_id), |(provider, model)| (Some(provider), model));
		let mut matches = sqlx::query_as!(
			ResolvedModel,
			r#"
			SELECT DISTINCT
				m.id,
				m.model_id AS engine_model_id,
				COALESCE(c.max_output_tokens, m.max_tokens) AS max_tokens,
				COALESCE(c.context_length, m.context_length) AS context_length
			FROM models m
			JOIN providers p ON p.id = m.provider_id
			LEFT JOIN model_configs c ON c.model_id = m.id AND c.owner_id IS NULL
			WHERE m.model_id = $3
			  AND ($4::text IS NULL OR LOWER(p.name) = LOWER($4))
			  AND m.is_enabled = TRUE
			  AND p.is_enabled = TRUE
			  AND EXISTS (
				SELECT 1
				FROM team_members tm
				JOIN teams t ON t.id = tm.team_id
				LEFT JOIN team_model_access model_access
					ON model_access.team_id = t.id AND model_access.model_id = m.id
				LEFT JOIN team_model_access provider_access
					ON provider_access.team_id = t.id AND provider_access.provider_id = p.id
				WHERE tm.user_id = $1
				  AND t.id = $2
				  AND (t.allow_all_models OR model_access.id IS NOT NULL OR provider_access.id IS NOT NULL)
			  )
			LIMIT 2
			"#,
			context.user_id,
			context.team_id,
			model_id,
			provider_name
		)
		.fetch_all(pool)
		.await?;
		Ok(if matches.len() == 1 { matches.pop() } else { None })
	}

	/// Resolves one accessible model into the bounds gateway inference needs.
	///
	/// # Errors
	///
	/// Returns [`GatewayModelAccessError::NotFound`] for inaccessible or ambiguous
	/// models and a database error when model access cannot be queried.
	pub async fn resolve_inference(pool: &PgPool, context: &GatewayAuthContext, requested_id: &str) -> Result<GatewayInference, GatewayModelAccessError> {
		let model = Self::resolve_model(pool, context, requested_id).await?.ok_or(GatewayModelAccessError::NotFound)?;
		Ok(model.into())
	}

	/// Reserves the estimated cost of a resolved model against the key's budgets.
	///
	/// Free models bypass budget enforcement. Paid requests are admitted while
	/// every blocking budget has room, and the reservation travels with the
	/// returned inference.
	///
	/// # Errors
	///
	/// Returns [`GatewayModelAccessError::BudgetExceeded`] for blocked budgets,
	/// [`GatewayModelAccessError::BudgetUnavailable`] when reservation stopped, and
	/// a database error when policy evaluation fails.
	pub async fn authorize_inference(
		pool: &PgPool,
		ledger: &BudgetLedger,
		context: &GatewayAuthContext,
		mut inference: GatewayInference,
		tokens: TokenEstimate,
	) -> Result<GatewayInference, GatewayModelAccessError> {
		inference.budget = ledger.reserve_for_model(pool, context.budget_subject(), &inference.model_id, tokens).await?;
		Ok(inference)
	}
}

struct ParsedToken<'a> {
	key_id: Uuid,
	secret: &'a str,
}

fn parse_token(token: &str) -> Option<ParsedToken<'_>> {
	let remainder = token.strip_prefix("oxc_")?;
	let (key_id, secret) = remainder.split_once('_')?;
	(secret.len() >= 32).then_some(ParsedToken {
		key_id: Uuid::parse_str(key_id).ok()?,
		secret,
	})
}
