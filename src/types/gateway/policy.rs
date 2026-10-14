use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

// Request Types

#[derive(Clone, Debug)]
/// Policy and usage sent to an admission backend.
pub struct GatewayAdmissionRequest {
	pub team_id: Uuid,
	pub key_id: Uuid,
	pub policy: Arc<GatewayRatePolicy>,
	pub route_class: GatewayRouteClass,
	pub tokens: u64,
}

// Response Types

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
/// A rejected admission or a policy enforcement failure.
pub enum GatewayLimitError {
	/// The enforcement service stopped or could not answer.
	#[error("gateway_limiter_unavailable")]
	Unavailable,
	/// The enforcement service has too many pending admissions.
	#[error("gateway_limiter_overloaded")]
	Overloaded,
	/// Stored policy or model data cannot be enforced.
	#[error("gateway_limiter_misconfigured")]
	Misconfigured,
	/// A rule or concurrency cap rejected the request.
	#[error("gateway_limit_exceeded")]
	Rejected {
		dimension: GatewayLimitDimension,
		snapshot: Box<GatewayLimitSnapshot>,
		retry_after: Duration,
	},
}

// Internal Types

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
/// Quota family selected by the authenticated endpoint.
pub enum GatewayRouteClass {
	Inference,
	Utility,
}

#[derive(Clone, Copy, Debug)]
/// An authenticated endpoint and the quota family it consumes.
pub struct GatewayRoute {
	pub class: GatewayRouteClass,
	pub path: &'static str,
}

impl GatewayRoute {
	pub const MODELS: Self = Self {
		class: GatewayRouteClass::Utility,
		path: "/models",
	};
	pub const CHAT: Self = Self {
		class: GatewayRouteClass::Inference,
		path: "/chat/completions",
	};
	pub const RESPONSES: Self = Self {
		class: GatewayRouteClass::Inference,
		path: "/responses",
	};
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
/// Resource a refilling rule can meter.
pub enum GatewayRuleMetric {
	Requests,
	Tokens,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
/// Protocol-independent resource that can reject an admission.
pub enum GatewayLimitDimension {
	Requests,
	Tokens,
	Concurrency,
}

impl From<GatewayRuleMetric> for GatewayLimitDimension {
	fn from(metric: GatewayRuleMetric) -> Self {
		match metric {
			GatewayRuleMetric::Requests => Self::Requests,
			GatewayRuleMetric::Tokens => Self::Tokens,
		}
	}
}

#[derive(Clone, Debug, Deserialize)]
/// One enabled continuously refilling policy window.
pub struct GatewayRateRule {
	pub id: Uuid,
	pub route_class: GatewayRouteClass,
	pub metric: GatewayRuleMetric,
	#[serde(deserialize_with = "positive_capacity")]
	pub capacity: u64,
	#[serde(deserialize_with = "positive_period")]
	pub refill_period_seconds: u32,
}

#[derive(Clone, Debug, Default)]
/// Independently enforced team and key limits.
pub struct GatewayRatePolicy {
	pub team_rules: Vec<GatewayRateRule>,
	pub key_rules: Vec<GatewayRateRule>,
	pub team_concurrency: Option<u32>,
	pub key_concurrency: Option<u32>,
}

impl GatewayRatePolicy {
	/// Builds a validated policy from stored rules and concurrency caps.
	///
	/// # Errors
	/// Returns [`GatewayLimitError::Misconfigured`] for a malformed rule or cap.
	pub fn from_stored(
		team_rules: Vec<GatewayRateRule>,
		key_rules: Vec<GatewayRateRule>,
		team_concurrency: Option<i32>,
		key_concurrency: Option<i32>,
	) -> Result<Self, GatewayLimitError> {
		let policy = Self {
			team_rules,
			key_rules,
			team_concurrency: concurrency_cap(team_concurrency)?,
			key_concurrency: concurrency_cap(key_concurrency)?,
		};
		policy.validate()?;
		Ok(policy)
	}

	/// Checks the policy against the domain the limiter can enforce.
	///
	/// # Errors
	/// Returns [`GatewayLimitError::Misconfigured`] for a malformed rule or cap.
	pub fn validate(&self) -> Result<(), GatewayLimitError> {
		let invalid_rule = self.team_rules.iter().chain(&self.key_rules).any(|rule| !rule.is_valid());
		if invalid_rule || self.team_concurrency == Some(0) || self.key_concurrency == Some(0) {
			return Err(GatewayLimitError::Misconfigured);
		}
		Ok(())
	}

	#[must_use]
	/// Reports whether inference needs a token estimate.
	pub fn has_token_rules(&self) -> bool {
		self.team_rules
			.iter()
			.chain(&self.key_rules)
			.any(|rule| rule.matches(GatewayRouteClass::Inference) && rule.metric == GatewayRuleMetric::Tokens)
	}

	#[must_use]
	/// Reports whether a stopped backend must fail this route closed.
	pub fn applies(&self, class: GatewayRouteClass) -> bool {
		self.team_rules.iter().chain(&self.key_rules).any(|rule| rule.matches(class))
			|| (class == GatewayRouteClass::Inference && (self.team_concurrency.is_some() || self.key_concurrency.is_some()))
	}
}

impl GatewayRateRule {
	pub(super) fn matches(&self, class: GatewayRouteClass) -> bool {
		self.route_class == class && (class == GatewayRouteClass::Inference || self.metric == GatewayRuleMetric::Requests)
	}

	fn is_valid(&self) -> bool {
		self.capacity > 0
			&& self.capacity <= i64::MAX.unsigned_abs()
			&& self.refill_period_seconds > 0
			&& self.refill_period_seconds <= i32::MAX.unsigned_abs()
			&& !(self.route_class == GatewayRouteClass::Utility && self.metric == GatewayRuleMetric::Tokens)
	}
}

fn concurrency_cap(value: Option<i32>) -> Result<Option<u32>, GatewayLimitError> {
	value
		.map(|value| u32::try_from(value).ok().filter(|value| *value > 0).ok_or(GatewayLimitError::Misconfigured))
		.transpose()
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
