use crate::types::providers::{Provider, ProviderBillingMetric, ProviderBillingStatus, ProviderKind};
use crate::utils::encryption::decrypt_api_key;
use reqwest::{Client, Response, StatusCode};
use sqlx::PgPool;
use std::sync::OnceLock;
use std::time::Duration;
use thiserror::Error;

pub mod openai;
pub mod openrouter;

static CLIENT: OnceLock<Client> = OnceLock::new();

#[derive(Debug, Error, Clone, Copy)]
pub enum ProviderBillingError {
	#[error("unsupported")]
	Unsupported,
	#[error("missing_configuration")]
	MissingConfiguration,
	#[error("unauthorized")]
	Unauthorized,
	#[error("rate_limited")]
	RateLimited,
	#[error("invalid_response")]
	InvalidResponse,
	#[error("request_failed")]
	RequestFailed,
}

impl ProviderBillingError {
	fn from_request(error: reqwest::Error) -> Self {
		let _ = error;
		Self::RequestFailed
	}

	#[must_use]
	pub fn code(self) -> &'static str {
		match self {
			Self::Unsupported => "UNSUPPORTED",
			Self::MissingConfiguration => "MISSING_CONFIGURATION",
			Self::Unauthorized => "UNAUTHORIZED",
			Self::RateLimited => "RATE_LIMITED",
			Self::InvalidResponse => "INVALID_RESPONSE",
			Self::RequestFailed => "REQUEST_FAILED",
		}
	}

	#[must_use]
	fn status(self) -> ProviderBillingStatus {
		match self {
			Self::Unsupported => ProviderBillingStatus::Unsupported,
			Self::MissingConfiguration => ProviderBillingStatus::NotConfigured,
			Self::Unauthorized => ProviderBillingStatus::Unauthorized,
			Self::RateLimited | Self::InvalidResponse | Self::RequestFailed => ProviderBillingStatus::UpstreamError,
		}
	}
}

fn client() -> Result<&'static Client, ProviderBillingError> {
	if let Some(client) = CLIENT.get() {
		return Ok(client);
	}
	let built = Client::builder()
		.timeout(Duration::from_secs(10))
		.build()
		.map_err(|_| ProviderBillingError::RequestFailed)?;
	let _ = CLIENT.set(built);
	CLIENT.get().ok_or(ProviderBillingError::RequestFailed)
}

fn response_error(response: Response) -> Result<Response, ProviderBillingError> {
	match response.status() {
		StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderBillingError::Unauthorized),
		StatusCode::TOO_MANY_REQUESTS => Err(ProviderBillingError::RateLimited),
		status if !status.is_success() => Err(ProviderBillingError::RequestFailed),
		_ => Ok(response),
	}
}

pub async fn refresh_provider_billing(pool: &PgPool, provider: &Provider) -> Result<ProviderBillingMetric, ProviderBillingError> {
	let connection = Provider::find_billing_connection_for_admin(pool, &provider.id)
		.await
		.map_err(|_| ProviderBillingError::RequestFailed)?
		.ok_or(ProviderBillingError::MissingConfiguration)?;
	if !connection.is_enabled {
		return Err(ProviderBillingError::MissingConfiguration);
	}
	let result = async {
		match provider.kind {
			ProviderKind::Openrouter => {
				if let Some(stored) = connection.credential.as_deref() {
					let key = decrypt_api_key(stored);
					openrouter::fetch_account_metric(client()?, &key).await
				} else {
					let key = provider.api_key.as_deref().ok_or(ProviderBillingError::MissingConfiguration).map(decrypt_api_key)?;
					openrouter::fetch_key_metric(client()?, &key).await
				}
			}
			ProviderKind::Openai => {
				let key = connection
					.credential
					.as_deref()
					.ok_or(ProviderBillingError::MissingConfiguration)
					.map(decrypt_api_key)?;
				let project_id = connection
					.external_scope_id
					.as_deref()
					.filter(|value| !value.trim().is_empty())
					.ok_or(ProviderBillingError::MissingConfiguration)?;
				openai::fetch_metric(client()?, &key, project_id).await
			}
			ProviderKind::Anthropic | ProviderKind::Google | ProviderKind::OpenaiCompat | ProviderKind::Custom => Err(ProviderBillingError::Unsupported),
		}
	}
	.await;
	match result {
		Ok(metric) => {
			Provider::save_billing_snapshot(pool, &provider.id, &metric)
				.await
				.map_err(|_| ProviderBillingError::RequestFailed)?;
			Ok(metric)
		}
		Err(error) => {
			let _ = Provider::mark_billing_refresh_failure(pool, &provider.id, error.status(), error.code()).await;
			Err(error)
		}
	}
}
