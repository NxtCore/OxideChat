use super::ProviderKind;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
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
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
	pub kind: Option<ProviderKind>,
	pub name: Option<String>,
	pub base_url: Option<String>,
	pub api_key: Option<String>,
	pub extra_headers: Option<Value>,
	pub is_enabled: Option<bool>,
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
