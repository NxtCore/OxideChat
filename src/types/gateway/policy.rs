use serde::Deserialize;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
/// Quota family selected by the authenticated endpoint.
pub enum GatewayRouteClass {
	Inference,
	Utility,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
/// Protocol-independent admission resource.
pub enum GatewayLimitDimension {
	Requests,
	Tokens,
	Concurrency,
}

#[derive(Clone, Debug, Deserialize)]
/// One enabled continuously refilling policy window.
pub struct GatewayRateRule {
	pub id: Uuid,
	pub route_class: GatewayRouteClass,
	pub metric: GatewayLimitDimension,
	#[serde(deserialize_with = "positive_capacity")]
	pub capacity: u64,
	#[serde(deserialize_with = "positive_period")]
	pub refill_period_seconds: u32,
}

#[derive(Clone, Debug, Default)]
/// Independently enforced project and key limits.
pub struct GatewayRatePolicy {
	pub project_rules: Vec<GatewayRateRule>,
	pub key_rules: Vec<GatewayRateRule>,
	pub project_concurrency: Option<u32>,
	pub key_concurrency: Option<u32>,
}

impl GatewayRatePolicy {
	#[must_use]
	/// Reports whether inference needs a token estimate.
	pub fn has_token_rules(&self) -> bool {
		self.project_rules
			.iter()
			.chain(&self.key_rules)
			.any(|rule| rule.matches(GatewayRouteClass::Inference) && rule.metric == GatewayLimitDimension::Tokens)
	}

	/// Checks runtime values against the stored policy domain.
	///
	/// # Errors
	/// Returns unavailable for a malformed rule or concurrency cap.
	pub fn validate(&self) -> Result<(), GatewayLimitError> {
		if self.project_rules.iter().chain(&self.key_rules).any(|rule| {
			rule.capacity == 0
				|| rule.capacity > i64::MAX as u64
				|| rule.refill_period_seconds == 0
				|| rule.refill_period_seconds > i32::MAX as u32
				|| rule.metric == GatewayLimitDimension::Concurrency
		}) || self.project_concurrency == Some(0)
			|| self.key_concurrency == Some(0)
		{
			return Err(GatewayLimitError::Unavailable);
		}
		Ok(())
	}

	#[must_use]
	/// Reports whether a stopped backend must fail this route closed.
	pub fn applies(&self, class: GatewayRouteClass) -> bool {
		self.project_rules.iter().chain(&self.key_rules).any(|rule| rule.matches(class))
			|| (class == GatewayRouteClass::Inference && (self.project_concurrency.is_some() || self.key_concurrency.is_some()))
	}
}

impl GatewayRateRule {
	pub(super) fn matches(&self, class: GatewayRouteClass) -> bool {
		self.route_class == class && (class == GatewayRouteClass::Inference || self.metric == GatewayLimitDimension::Requests)
	}
}

#[derive(Clone, Debug)]
/// The binding rule's remaining whole units and time until fully refilled.
pub struct GatewayMetricSnapshot {
	pub rule_id: Uuid,
	pub capacity: u64,
	pub remaining: u64,
	pub reset_after: Duration,
}

#[derive(Clone, Debug, Default)]
/// Neutral capacities for protocol-specific response rendering.
pub struct GatewayLimitSnapshot {
	pub requests: Option<GatewayMetricSnapshot>,
	pub tokens: Option<GatewayMetricSnapshot>,
	pub concurrency: Option<(u32, u32)>,
}

#[derive(Debug, thiserror::Error)]
/// A rejected admission or unavailable policy enforcement service.
pub enum GatewayLimitError {
	#[error("gateway_limiter_unavailable")]
	Unavailable,
	#[error("gateway_limit_exceeded")]
	Rejected {
		dimension: GatewayLimitDimension,
		snapshot: GatewayLimitSnapshot,
		retry_after: Duration,
	},
}

#[derive(Clone, Debug)]
/// Owned policy snapshot sent to an admission backend.
pub struct GatewayAdmissionRequest {
	pub project_id: Uuid,
	pub key_id: Uuid,
	pub policy: GatewayRatePolicy,
	pub route_class: GatewayRouteClass,
	pub tokens: u64,
}

fn positive_capacity<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
	let value = i64::deserialize(deserializer)?;
	u64::try_from(value)
		.ok()
		.filter(|value| *value > 0)
		.ok_or_else(|| serde::de::Error::custom("invalid gateway capacity"))
}

fn positive_period<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
	let value = i32::deserialize(deserializer)?;
	u32::try_from(value)
		.ok()
		.filter(|value| *value > 0)
		.ok_or_else(|| serde::de::Error::custom("invalid gateway refill period"))
}
