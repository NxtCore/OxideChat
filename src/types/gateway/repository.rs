use super::{GatewayAuthContext, GatewayAuthError, GatewayCredential, OpenAiModel};
use crate::utils::auth::{hash_password, verify_password};
use chrono::Utc;
use sqlx::PgPool;
use std::sync::LazyLock;
use uuid::Uuid;

static DUMMY_SECRET_HASH: LazyLock<String> = LazyLock::new(|| hash_password("oxide-gateway-invalid-secret").unwrap_or_default());

impl GatewayCredential {
	pub async fn authenticate(pool: &PgPool, token: &str) -> Result<GatewayAuthContext, GatewayAuthError> {
		let key_id = parse_key_id(token);
		let credential = match key_id {
			Some(id) => Self::find(pool, &id).await.map_err(|_| GatewayAuthError::Unavailable)?,
			None => None,
		};
		let hash = credential.as_ref().map_or(DUMMY_SECRET_HASH.as_str(), |value| value.secret_hash.as_str());
		let secret = token.rsplit_once('_').map_or("", |(_, value)| value).to_owned();
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
		sqlx::query("UPDATE gateway_api_keys SET last_used_at = NOW() WHERE id = $1")
			.bind(credential.key_id)
			.execute(pool)
			.await
			.map_err(|_| GatewayAuthError::Unavailable)?;
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

impl OpenAiModel {
	pub async fn list_for_user(pool: &PgPool, user_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>(
			r#"
			SELECT
				m.model_id AS id,
				'model'::text AS object,
				EXTRACT(EPOCH FROM m.created_at)::bigint AS created,
				p.name AS owned_by
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
				  AND (t.allow_all_models OR model_access.id IS NOT NULL OR provider_access.id IS NOT NULL)
			  )
			ORDER BY m.model_id
			"#,
		)
		.bind(user_id)
		.fetch_all(pool)
		.await
	}
}

fn parse_key_id(token: &str) -> Option<Uuid> {
	let mut parts = token.splitn(3, '_');
	(parts.next() == Some("oxc"))
		.then(|| parts.next())
		.flatten()
		.and_then(|value| Uuid::parse_str(value).ok())
		.filter(|_| parts.next().is_some_and(|secret| secret.len() >= 32))
}
