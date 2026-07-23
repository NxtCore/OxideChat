use super::{ProviderBillingError, response_error};
use crate::types::providers::{ProviderBillingMetric, ProviderBillingMetricKind};
use chrono::{DateTime, Utc};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;

const BASE_URL: &str = "https://openrouter.ai";

#[derive(Deserialize)]
struct KeyEnvelope {
	data: KeyData,
}

#[derive(Deserialize)]
struct KeyData {
	limit: Option<Decimal>,
	limit_remaining: Option<Decimal>,
	limit_reset: Option<String>,
	#[serde(default)]
	usage_monthly: Decimal,
}

#[derive(Deserialize)]
struct CreditsEnvelope {
	data: CreditsData,
}

#[derive(Deserialize)]
struct CreditsData {
	total_credits: Decimal,
	total_usage: Decimal,
}

pub async fn fetch_key_metric(client: &Client, key: &str) -> Result<ProviderBillingMetric, ProviderBillingError> {
	fetch_key_metric_at(client, key, BASE_URL).await
}

pub async fn fetch_account_metric(client: &Client, key: &str) -> Result<ProviderBillingMetric, ProviderBillingError> {
	fetch_account_metric_at(client, key, BASE_URL).await
}

async fn fetch_key_metric_at(client: &Client, key: &str, base_url: &str) -> Result<ProviderBillingMetric, ProviderBillingError> {
	let response = client
		.get(format!("{base_url}/api/v1/key"))
		.bearer_auth(key)
		.send()
		.await
		.map_err(ProviderBillingError::from_request)?;
	let response = response_error(response)?;
	let payload: KeyEnvelope = response.json().await.map_err(|_| ProviderBillingError::InvalidResponse)?;
	let period_end = payload
		.data
		.limit_reset
		.as_deref()
		.and_then(|value| DateTime::parse_from_rfc3339(value).ok())
		.map(|value| value.with_timezone(&Utc));
	let metric_kind = if payload.data.limit.is_some() {
		ProviderBillingMetricKind::KeyLimit
	} else {
		ProviderBillingMetricKind::SpendOnly
	};
	Ok(ProviderBillingMetric {
		metric_kind,
		currency: "USD".to_string(),
		period_start: None,
		period_end,
		limit_amount: payload.data.limit,
		spent_amount: Some(payload.data.usage_monthly),
		remaining_amount: payload.data.limit_remaining,
		is_hard_limit: payload.data.limit.is_some(),
		thresholds: Vec::new(),
		details: serde_json::json!({"scope": "key"}),
		fetched_at: Utc::now(),
	})
}

async fn fetch_account_metric_at(client: &Client, key: &str, base_url: &str) -> Result<ProviderBillingMetric, ProviderBillingError> {
	let response = client
		.get(format!("{base_url}/api/v1/credits"))
		.bearer_auth(key)
		.send()
		.await
		.map_err(ProviderBillingError::from_request)?;
	let response = response_error(response)?;
	let payload: CreditsEnvelope = response.json().await.map_err(|_| ProviderBillingError::InvalidResponse)?;
	let remaining = (payload.data.total_credits - payload.data.total_usage).max(Decimal::ZERO);
	Ok(ProviderBillingMetric {
		metric_kind: ProviderBillingMetricKind::CreditBalance,
		currency: "USD".to_string(),
		period_start: None,
		period_end: None,
		limit_amount: None,
		spent_amount: None,
		remaining_amount: Some(remaining),
		is_hard_limit: true,
		thresholds: Vec::new(),
		details: serde_json::json!({"scope": "account"}),
		fetched_at: Utc::now(),
	})
}
