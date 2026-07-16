use super::{ProviderBillingError, response_error};
use crate::types::providers::{ProviderBillingMetric, ProviderBillingMetricKind};
use chrono::{Datelike, TimeZone, Utc};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;

const BASE_URL: &str = "https://api.openai.com";

#[derive(Deserialize)]
struct CostPage {
	#[serde(default)]
	data: Vec<CostBucket>,
	#[serde(default)]
	has_more: bool,
	next_page: Option<String>,
}

#[derive(Deserialize)]
struct CostBucket {
	#[serde(default)]
	results: Vec<CostResult>,
}

#[derive(Deserialize)]
struct CostResult {
	project_id: Option<String>,
	amount: CostAmount,
}

#[derive(Deserialize)]
struct CostAmount {
	value: Decimal,
	currency: String,
}

#[derive(Deserialize)]
struct AlertPage {
	#[serde(default)]
	data: Vec<SpendAlert>,
	#[serde(default)]
	has_more: bool,
	next_page: Option<String>,
}

#[derive(Deserialize)]
struct SpendAlert {
	threshold_amount: Decimal,
}

pub async fn fetch_metric(client: &Client, key: &str, project_id: &str) -> Result<ProviderBillingMetric, ProviderBillingError> {
	fetch_metric_at(client, key, project_id, BASE_URL).await
}

async fn fetch_metric_at(client: &Client, key: &str, project_id: &str, base_url: &str) -> Result<ProviderBillingMetric, ProviderBillingError> {
	let now = Utc::now();
	let start = Utc
		.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
		.single()
		.ok_or(ProviderBillingError::InvalidResponse)?;
	let end = if now.month() == 12 {
		Utc.with_ymd_and_hms(now.year() + 1, 1, 1, 0, 0, 0).single()
	} else {
		Utc.with_ymd_and_hms(now.year(), now.month() + 1, 1, 0, 0, 0).single()
	}
	.ok_or(ProviderBillingError::InvalidResponse)?;
	let (spent, currency) = fetch_costs(client, key, project_id, base_url, start.timestamp(), end.timestamp()).await?;
	let mut thresholds = fetch_thresholds(client, key, project_id, base_url).await?;
	thresholds.sort();
	thresholds.dedup();
	let limit = thresholds.last().copied();
	let remaining = limit.map(|value| (value - spent).max(Decimal::ZERO));
	Ok(ProviderBillingMetric {
		metric_kind: if limit.is_some() {
			ProviderBillingMetricKind::SpendThreshold
		} else {
			ProviderBillingMetricKind::SpendOnly
		},
		currency,
		period_start: Some(start),
		period_end: Some(end),
		limit_amount: limit,
		spent_amount: Some(spent),
		remaining_amount: remaining,
		is_hard_limit: false,
		thresholds,
		details: serde_json::json!({"scope": "project"}),
		fetched_at: Utc::now(),
	})
}

async fn fetch_costs(client: &Client, key: &str, project_id: &str, base_url: &str, start: i64, end: i64) -> Result<(Decimal, String), ProviderBillingError> {
	let mut page: Option<String> = None;
	let mut total = Decimal::ZERO;
	let mut currency = "USD".to_string();
	loop {
		let mut request = client.get(format!("{base_url}/v1/organization/costs")).bearer_auth(key).query(&[
			("start_time", start.to_string()),
			("end_time", end.to_string()),
			("bucket_width", "1d".to_string()),
			("project_ids[]", project_id.to_string()),
		]);
		if let Some(value) = page.as_deref() {
			request = request.query(&[("page", value)]);
		}
		let response = request.send().await.map_err(ProviderBillingError::from_request)?;
		let payload: CostPage = response_error(response)?.json().await.map_err(|_| ProviderBillingError::InvalidResponse)?;
		for result in payload.data.into_iter().flat_map(|bucket| bucket.results) {
			if result.project_id.as_deref().is_none_or(|id| id == project_id) {
				total += result.amount.value;
				currency = result.amount.currency.to_uppercase();
			}
		}
		if !payload.has_more {
			break;
		}
		page = payload.next_page;
		if page.is_none() {
			return Err(ProviderBillingError::InvalidResponse);
		}
	}
	Ok((total, currency))
}

async fn fetch_thresholds(client: &Client, key: &str, project_id: &str, base_url: &str) -> Result<Vec<Decimal>, ProviderBillingError> {
	let mut page: Option<String> = None;
	let mut thresholds = Vec::new();
	loop {
		let mut request = client.get(format!("{base_url}/v1/organization/projects/{project_id}/spend_alerts")).bearer_auth(key);
		if let Some(value) = page.as_deref() {
			request = request.query(&[("page", value)]);
		}
		let response = request.send().await.map_err(ProviderBillingError::from_request)?;
		let payload: AlertPage = response_error(response)?.json().await.map_err(|_| ProviderBillingError::InvalidResponse)?;
		thresholds.extend(payload.data.into_iter().map(|alert| alert.threshold_amount / Decimal::from(100)));
		if !payload.has_more {
			break;
		}
		page = payload.next_page;
		if page.is_none() {
			return Err(ProviderBillingError::InvalidResponse);
		}
	}
	Ok(thresholds)
}
