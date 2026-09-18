use super::{GatewayAuthContext, GatewayAuthError, GatewayCredential, GatewayInference, GatewayModel, GatewayModelAccessError};
use super::{GatewayRatePolicy, GatewayRateRule};
use crate::types::Budget;
use crate::types::models::ModelPricing;
use crate::utils::auth::{hash_password, verify_password};
use chrono::Utc;
use omniference::types::providers::OpenAIModel;
use sqlx::{PgPool, types::Json};
use std::sync::LazyLock;
use uuid::Uuid;

static DUMMY_SECRET_HASH: LazyLock<String> = LazyLock::new(|| hash_password("oxide-gateway-invalid-secret").unwrap_or_default());

impl GatewayCredential {
	/// Authenticates a gateway token and returns its enabled project context.
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
			|| !credential.project_enabled
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
		let policy = GatewayRatePolicy {
			project_rules: credential.project_rules.0,
			key_rules: credential.key_rules.0,
			project_concurrency: concurrency_limit(credential.project_concurrency)?,
			key_concurrency: concurrency_limit(credential.key_concurrency)?,
		};
		policy.validate().map_err(|error| {
			tracing::error!(%error, project_id = %credential.project_id, key_id = %credential.key_id, "invalid gateway rate policy");
			GatewayAuthError::Unavailable
		})?;
		Ok(GatewayAuthContext {
			key_id: credential.key_id,
			project_id: credential.project_id,
			user_id: credential.user_id,
			team_id: credential.team_id,
			project_name: credential.project_name,
			scopes: credential.scopes.0,
			policy,
		})
	}

	async fn find(pool: &PgPool, key_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as!(
			Self,
			r#"
			SELECT
				k.id AS "key_id!",
				k.project_id,
				p.owner_id AS "user_id!",
				p.team_id,
				p.name AS "project_name!",
				k.secret_hash,
				k.scopes AS "scopes!: Json<Vec<String>>",
				k.is_enabled AS "key_enabled!",
				p.is_enabled AS "project_enabled!",
				k.expires_at,
				k.revoked_at,
				p.max_concurrent_requests AS project_concurrency,
				k.max_concurrent_requests AS key_concurrency,
				COALESCE((SELECT jsonb_agg(jsonb_build_object('id', r.id, 'route_class', r.route_class, 'metric', r.metric, 'capacity', r.capacity, 'refill_period_seconds', r.refill_period_seconds) ORDER BY r.id)
				 FROM gateway_rate_limit_rules r WHERE r.project_id = p.id AND r.is_enabled), '[]'::jsonb) AS "project_rules!: Json<Vec<GatewayRateRule>>",
				COALESCE((SELECT jsonb_agg(jsonb_build_object('id', r.id, 'route_class', r.route_class, 'metric', r.metric, 'capacity', r.capacity, 'refill_period_seconds', r.refill_period_seconds) ORDER BY r.id)
				 FROM gateway_rate_limit_rules r WHERE r.api_key_id = k.id AND r.is_enabled), '[]'::jsonb) AS "key_rules!: Json<Vec<GatewayRateRule>>"
			FROM gateway_api_keys k
			JOIN gateway_projects p ON p.id = k.project_id
			WHERE k.id = $1
			"#,
			key_id
		)
		.fetch_optional(pool)
		.await
	}
}

impl GatewayModel {
	/// Lists enabled models accessible through the authenticated user's eligible teams.
	///
	/// A team-bound project is filtered to that team. Models granted through
	/// multiple teams are returned only once.
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
				  AND ($2::uuid IS NULL OR t.id = $2)
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
		let (provider_name, model_id) = requested_id.split_once('/').map_or((None, requested_id), |(provider, model)| (Some(provider), model));
		let matches = sqlx::query_scalar!(
			r#"
			SELECT DISTINCT m.id
			FROM models m
			JOIN providers p ON p.id = m.provider_id
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
				  AND ($2::uuid IS NULL OR t.id = $2)
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
		Ok((matches.len() == 1).then(|| matches[0]))
	}

	/// Resolves model access and checks budget admission.
	///
	/// Free models bypass budget enforcement. Paid requests are admitted while
	/// every blocking budget remains below its configured amount.
	///
	/// # Errors
	///
	/// Returns [`GatewayModelAccessError::NotFound`] for inaccessible or
	/// ambiguous models, [`GatewayModelAccessError::BudgetExceeded`] for blocked
	/// budgets, and a database error when policy evaluation fails.
	pub async fn authorize_inference(pool: &PgPool, context: &GatewayAuthContext, requested_id: &str) -> Result<GatewayInference, GatewayModelAccessError> {
		let model_id = Self::resolve_accessible(pool, context, requested_id)
			.await?
			.ok_or(GatewayModelAccessError::NotFound)?;
		if !ModelPricing::is_free(pool, &model_id).await? && !Budget::allows_inference(pool, &context.user_id).await? {
			return Err(GatewayModelAccessError::BudgetExceeded);
		}
		if !context.policy.has_token_rules() {
			return Ok(GatewayInference {
				model_id,
				max_output_tokens: None,
				context_length: None,
			});
		}
		let limits = sqlx::query_as!(
			super::rows::GatewayModelLimits,
			"SELECT COALESCE(c.max_output_tokens, m.max_tokens) AS max_tokens,
			 COALESCE(c.context_length, m.context_length) AS context_length
			 FROM models m LEFT JOIN model_configs c ON c.model_id = m.id AND c.owner_id IS NULL WHERE m.id = $1",
			model_id
		)
		.fetch_one(pool)
		.await?;
		Ok(GatewayInference {
			model_id,
			max_output_tokens: limits.max_tokens.and_then(|value| u32::try_from(value).ok()).filter(|value| *value > 0),
			context_length: limits.context_length.and_then(|value| u32::try_from(value).ok()).filter(|value| *value > 0),
		})
	}
}

fn concurrency_limit(value: Option<i32>) -> Result<Option<u32>, GatewayAuthError> {
	value
		.map(|value| {
			u32::try_from(value).ok().filter(|value| *value > 0).ok_or_else(|| {
				tracing::error!("invalid stored gateway concurrency limit");
				GatewayAuthError::Unavailable
			})
		})
		.transpose()
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
