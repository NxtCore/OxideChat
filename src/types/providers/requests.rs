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
	pub reported_cost: Option<ProviderReportedCost>,
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
	pub reported_cost: Option<Option<ProviderReportedCost>>,
}

/// The unit a provider reports its request cost in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ReportedCostUnit {
	Usd,
	Currency,
	Credits,
}

/// How a provider reports the cost of a request. Currencies are converted with the daily ECB
/// rates; credits use a fixed value set by an admin.
#[derive(Debug, Clone, PartialEq, serde::Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(optional_fields = nullable))]
pub struct ProviderReportedCost {
	pub unit: ReportedCostUnit,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub currency: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(type = "string"))]
	pub usd_per_credit: Option<rust_decimal::Decimal>,
	/// A body path such as `usage.cost`, or `header:<name>`; `None` detects the common fields.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub source: Option<String>,
}

const HEADER_SOURCE_PREFIX: &str = "header:";
const MAX_SOURCE_LEN: usize = 200;

fn valid_source(source: &str) -> bool {
	let valid_part = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
	source.len() <= MAX_SOURCE_LEN
		&& match source.strip_prefix(HEADER_SOURCE_PREFIX) {
			Some(name) => valid_part(name),
			None => source.split('.').all(valid_part),
		}
}

impl ProviderReportedCost {
	/// Returns `true` when the unit has what it needs and the source, if any, is well formed.
	#[must_use]
	pub fn is_valid(&self) -> bool {
		let unit_ok = match self.unit {
			ReportedCostUnit::Usd => true,
			ReportedCostUnit::Currency => self.currency.as_deref().is_some_and(|code| code.len() == 3 && code.chars().all(|c| c.is_ascii_uppercase())),
			ReportedCostUnit::Credits => self.usd_per_credit.is_some_and(|value| value > rust_decimal::Decimal::ZERO),
		};
		unit_ok && self.source.as_deref().is_none_or(valid_source)
	}

	/// The Omniference config for this provider, using `usd_per_unit` for currencies.
	/// Returns `None` when a currency has no rate yet, so the provider falls back to token pricing.
	#[must_use]
	pub fn to_omniference(&self, usd_per_unit: &std::collections::HashMap<String, rust_decimal::Decimal>) -> Option<omniference::reported_cost::ReportedCost> {
		use omniference::reported_cost::{CostSource, CostUnit, ReportedCost};
		use rust_decimal::prelude::ToPrimitive;
		let unit = match self.unit {
			ReportedCostUnit::Usd => CostUnit::Usd,
			ReportedCostUnit::Currency => {
				let code = self.currency.clone()?;
				CostUnit::Currency {
					usd_per_unit: usd_per_unit.get(&code)?.to_f64()?,
					code,
				}
			}
			ReportedCostUnit::Credits => CostUnit::Credits {
				usd_per_credit: self.usd_per_credit?.to_f64()?,
			},
		};
		let source = self.source.as_deref().map(|source| match source.strip_prefix(HEADER_SOURCE_PREFIX) {
			Some(name) => CostSource::Header { name: name.to_string() },
			None => CostSource::Body { path: source.to_string() },
		});
		Some(ReportedCost { source, unit })
	}
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
