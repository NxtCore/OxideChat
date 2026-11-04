use super::ProviderKind;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "UpdateProviderBillingPayload", optional_fields = nullable))]
pub struct UpdateProviderBillingRequest {
	pub credential: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
	pub kind: ProviderKind,
	pub name: String,
	pub base_url: String,
	pub api_key: Option<String>,
	#[serde(default)]
	pub extra_headers: Value,
	#[serde(default = "default_true")]
	pub is_enabled: bool,
	#[serde(default)]
	pub cost_conversion_factor: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
	pub kind: Option<ProviderKind>,
	pub name: Option<String>,
	pub base_url: Option<String>,
	pub api_key: Option<String>,
	pub extra_headers: Option<Value>,
	pub is_enabled: Option<bool>,
	#[serde(default, deserialize_with = "crate::types::deserialize_nullable_field")]
	pub cost_conversion_factor: Option<Option<rust_decimal::Decimal>>,
}

/// Returns `true` when an optional cost conversion factor is absent or positive.
#[must_use]
pub fn valid_cost_conversion(factor: Option<rust_decimal::Decimal>) -> bool {
	factor.is_none_or(|value| value > rust_decimal::Decimal::ZERO)
}

#[derive(Debug, Deserialize)]
pub struct TestProviderRequest {
	pub kind: ProviderKind,
	pub base_url: String,
	pub api_key: Option<String>,
	#[serde(default)]
	pub extra_headers: Value,
}

fn default_true() -> bool {
	true
}
