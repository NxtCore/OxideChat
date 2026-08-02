use super::{GatewayAuthContext, GatewayAuthError, GatewayCredential, GatewayModel};
use crate::utils::auth::{hash_password, verify_password};
use chrono::Utc;
use omniference::types::providers::OpenAIModel;
use sqlx::PgPool;
use std::sync::LazyLock;
use uuid::Uuid;

static DUMMY_SECRET_HASH: LazyLock<String> = LazyLock::new(|| hash_password("oxide-gateway-invalid-secret").unwrap_or_default());

impl GatewayCredential {
	pub async fn authenticate(pool: &PgPool, token: &str) -> Result<GatewayAuthContext, GatewayAuthError> {
		let parsed = parse_token(token);
		let credential = match parsed.as_ref() {
			Some(parsed) => Self::find(pool, &parsed.key_id).await.map_err(|_| GatewayAuthError::Unavailable)?,
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
		if let Err(error) =
			sqlx::query("UPDATE gateway_api_keys SET last_used_at = NOW() WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < NOW() - INTERVAL '1 minute')")
				.bind(credential.key_id)
				.execute(pool)
				.await
		{
			tracing::warn!(%error, key_id = %credential.key_id, "failed to update gateway API key usage timestamp");
		}
		Ok(GatewayAuthContext {
			key_id: credential.key_id,
			project_id: credential.project_id,
			user_id: credential.user_id,
			team_id: credential.team_id,
			project_name: credential.project_name,
			scopes: credential.scopes.0,
		})
	}

	async fn find(pool: &PgPool, key_id: &Uuid) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			SELECT
				k.id AS key_id,
				k.project_id,
				p.owner_id AS user_id,
				p.team_id,
				p.name AS project_name,
				k.secret_hash,
				k.scopes,
				k.is_enabled AS key_enabled,
				p.is_enabled AS project_enabled,
				k.expires_at,
				k.revoked_at
			FROM gateway_api_keys k
			JOIN gateway_projects p ON p.id = k.project_id
			WHERE k.id = $1
			"#,
		)
		.bind(key_id)
		.fetch_optional(pool)
		.await
	}
}

impl GatewayModel {
	pub async fn list_for_context(pool: &PgPool, context: &GatewayAuthContext) -> Result<Vec<OpenAIModel>, sqlx::Error> {
		let models = sqlx::query_as::<_, Self>(
			r#"
			SELECT
				p.name AS provider_name,
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
		)
		.bind(context.user_id)
		.bind(context.team_id)
		.fetch_all(pool)
		.await?;
		Ok(models.into_iter().map(OpenAIModel::from).collect())
	}

	pub async fn resolve_accessible(pool: &PgPool, context: &GatewayAuthContext, requested_id: &str) -> Result<Option<Uuid>, sqlx::Error> {
		let (provider_name, model_id) = requested_id.split_once('/').map_or((None, requested_id), |(provider, model)| (Some(provider), model));
		let matches = sqlx::query_scalar::<_, Uuid>(
			r#"
			SELECT m.id
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
		)
		.bind(context.user_id)
		.bind(context.team_id)
		.bind(model_id)
		.bind(provider_name)
		.fetch_all(pool)
		.await?;
		Ok((matches.len() == 1).then(|| matches[0]))
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
