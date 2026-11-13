//! Price sheets: a model's rates with context tiers, service tiers and time windows, in USD per
//! million tokens. One shape serves the catalog price, a manual override and the merged price.

use omniference::catalog::{ModelPricing as EnginePricing, PricingError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const DAY_MINUTES: u32 = 24 * 60;
const WEEK_MINUTES: u32 = 7 * DAY_MINUTES;

// Request and Response Types

/// Rates in USD per million tokens. An unset rate falls back to the layer below.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PriceRates {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(optional))]
	pub input: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(optional))]
	pub output: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(optional))]
	pub cache_read: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(optional))]
	pub cache_write: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(optional))]
	pub reasoning: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(optional))]
	pub input_audio: Option<f64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(optional))]
	pub output_audio: Option<f64>,
}

/// Rates that apply once the prompt has at least `min_context_tokens` input tokens.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PriceTier {
	pub min_context_tokens: u32,
	#[serde(flatten)]
	pub rates: PriceRates,
}

/// Rates for requests that run on one service tier (`flex`, `priority`, `batch`, ...).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PriceServiceTier {
	#[serde(flatten)]
	pub rates: PriceRates,
	#[serde(default)]
	pub tiers: Vec<PriceTier>,
}

/// A day of the week a time window starts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum PriceWeekday {
	Mon,
	Tue,
	Wed,
	Thu,
	Fri,
	Sat,
	Sun,
}

/// Rates for requests that start between `start` and `end` (`HH:MM`, `end` exclusive, wrapping past
/// midnight when it is not after `start`) on `days` (every day when empty), in `timezone`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PriceWindow {
	#[serde(default)]
	pub days: Vec<PriceWeekday>,
	pub start: String,
	pub end: String,
	#[serde(default = "utc")]
	pub timezone: String,
	#[serde(default)]
	pub rates: PriceRates,
}

/// A model price. As an override, every unset rate and every unset list (`null` or missing) falls
/// back to the catalog price; an empty list replaces the catalog's with none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PriceSheet {
	#[serde(flatten)]
	pub rates: PriceRates,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(optional))]
	pub tiers: Option<Vec<PriceTier>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(optional))]
	pub service_tiers: Option<BTreeMap<String, PriceServiceTier>>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	#[cfg_attr(test, ts(optional))]
	pub windows: Option<Vec<PriceWindow>>,
}

/// The lowest and highest rate a model can be billed at, in USD per million tokens.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PriceRange {
	pub min: f64,
	pub max: f64,
}

/// What a price looks like at a glance: input and output ranges and the special rates behind them.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PriceSummary {
	pub input: PriceRange,
	pub output: PriceRange,
	/// Thresholds of the context tiers, ascending.
	pub context_tiers: Vec<u32>,
	pub service_tiers: Vec<String>,
	pub windows: usize,
}

/// A price with its summary.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PricedSheet {
	pub price: PriceSheet,
	pub summary: Option<PriceSummary>,
}

/// Why a price sheet was refused.
#[derive(Debug, thiserror::Error)]
pub enum PriceSheetError {
	#[error("input and output rates are required when the catalog has no price")]
	MissingBaseRates,
	#[error("context tier thresholds must be above zero")]
	ZeroThreshold,
	#[error("invalid service tier name `{0}`")]
	InvalidServiceTier(String),
	#[error("time window {0} starts and ends at the same time")]
	EmptyWindow(usize),
	#[error("time windows {0} and {1} overlap")]
	OverlappingWindows(usize, usize),
	#[error(transparent)]
	Invalid(#[from] PricingError),
	#[error("model not found")]
	NotFound,
	#[error(transparent)]
	Database(#[from] sqlx::Error),
}

// Internal Types

fn utc() -> String {
	"UTC".to_string()
}

impl PriceSheetError {
	/// A stable reason the frontend translates.
	#[must_use]
	pub fn reason(&self) -> &'static str {
		match self {
			Self::MissingBaseRates => "missing_base_rates",
			Self::ZeroThreshold => "zero_threshold",
			Self::InvalidServiceTier(_) => "invalid_service_tier",
			Self::EmptyWindow(_) => "empty_window",
			Self::OverlappingWindows(..) => "overlapping_windows",
			Self::Invalid(PricingError::InvalidRate(_)) => "invalid_rate",
			Self::Invalid(PricingError::DuplicateTier(_)) => "duplicate_tier",
			Self::Invalid(PricingError::InvalidTime(_)) => "invalid_time",
			Self::Invalid(PricingError::InvalidTimezone(_)) => "invalid_timezone",
			Self::NotFound => "not_found",
			Self::Database(_) => "database",
		}
	}
}

impl From<PriceSheet> for PricedSheet {
	fn from(price: PriceSheet) -> Self {
		let summary = price.summary();
		Self { price, summary }
	}
}

impl PriceRates {
	fn or(&self, below: &Self) -> Self {
		Self {
			input: self.input.or(below.input),
			output: self.output.or(below.output),
			cache_read: self.cache_read.or(below.cache_read),
			cache_write: self.cache_write.or(below.cache_write),
			reasoning: self.reasoning.or(below.reasoning),
			input_audio: self.input_audio.or(below.input_audio),
			output_audio: self.output_audio.or(below.output_audio),
		}
	}
}

impl PriceSheet {
	/// The catalog price as a sheet.
	#[must_use]
	pub fn from_engine(pricing: &EnginePricing) -> Self {
		serde_json::to_value(pricing).and_then(serde_json::from_value).unwrap_or_default()
	}

	/// The price Omniference bills with, or `None` without an input and output rate.
	#[must_use]
	pub fn to_engine(&self) -> Option<EnginePricing> {
		self.rates.input?;
		self.rates.output?;
		serde_json::to_value(self).and_then(serde_json::from_value).ok()
	}

	/// This override on top of `catalog`: each rate and each list it leaves unset comes from the catalog.
	#[must_use]
	pub fn over(&self, catalog: Option<&Self>) -> Self {
		let Some(catalog) = catalog else {
			return self.clone();
		};
		Self {
			rates: self.rates.or(&catalog.rates),
			tiers: self.tiers.clone().or_else(|| catalog.tiers.clone()),
			service_tiers: self.service_tiers.clone().or_else(|| catalog.service_tiers.clone()),
			windows: self.windows.clone().or_else(|| catalog.windows.clone()),
		}
	}

	/// Sorts context tiers by threshold and trims and lowercases service tier names and timezones.
	#[must_use]
	pub fn normalized(mut self) -> Self {
		if let Some(tiers) = self.tiers.as_mut() {
			tiers.sort_by_key(|tier| tier.min_context_tokens);
		}
		if let Some(services) = self.service_tiers.take() {
			let services = services
				.into_iter()
				.map(|(name, mut service)| {
					service.tiers.sort_by_key(|tier| tier.min_context_tokens);
					(name.trim().to_ascii_lowercase(), service)
				})
				.collect();
			self.service_tiers = Some(services);
		}
		if let Some(windows) = self.windows.as_mut() {
			for window in windows {
				window.timezone = window.timezone.trim().to_string();
				window.start = window.start.trim().to_string();
				window.end = window.end.trim().to_string();
			}
		}
		self
	}

	/// Checks an override before it is stored; `catalog` is the price it falls back to.
	///
	/// # Errors
	/// Returns the first problem: a negative or non-finite rate, a missing base rate without a
	/// catalog price, a zero or repeated tier threshold, a bad service tier name, or a time window
	/// with a bad time or timezone, no length, or that overlaps another one in the same timezone.
	pub fn validate(&self, catalog: Option<&Self>) -> Result<(), PriceSheetError> {
		let merged = self.over(catalog);
		if merged.rates.input.is_none() || merged.rates.output.is_none() {
			return Err(PriceSheetError::MissingBaseRates);
		}
		let mut own = self.clone();
		own.rates.input = Some(own.rates.input.unwrap_or_default());
		own.rates.output = Some(own.rates.output.unwrap_or_default());
		own.to_engine().ok_or(PriceSheetError::MissingBaseRates)?.validate()?;

		let services = self.service_tiers.iter().flatten();
		let thresholds = self.tiers.iter().flatten().chain(services.clone().flat_map(|(_, service)| &service.tiers));
		if thresholds.into_iter().any(|tier| tier.min_context_tokens == 0) {
			return Err(PriceSheetError::ZeroThreshold);
		}
		if let Some((name, _)) = services.into_iter().find(|(name, _)| !valid_service_tier(name)) {
			return Err(PriceSheetError::InvalidServiceTier(name.clone()));
		}
		validate_windows(self.windows.as_deref().unwrap_or_default())
	}

	/// Input and output ranges and the special rates of this price, or `None` without base rates.
	#[must_use]
	pub fn summary(&self) -> Option<PriceSummary> {
		let services = self.service_tiers.clone().unwrap_or_default();
		let tiers = self.tiers.as_deref().unwrap_or_default();
		let windows = self.windows.as_deref().unwrap_or_default();
		let layers: Vec<&PriceRates> = std::iter::once(&self.rates)
			.chain(tiers.iter().map(|tier| &tier.rates))
			.chain(services.values().flat_map(|service| std::iter::once(&service.rates).chain(service.tiers.iter().map(|tier| &tier.rates))))
			.chain(windows.iter().map(|window| &window.rates))
			.collect();
		let range = |pick: fn(&PriceRates) -> Option<f64>| -> Option<PriceRange> {
			let values: Vec<f64> = layers.iter().filter_map(|rates| pick(rates)).collect();
			pick(&self.rates)?;
			Some(PriceRange {
				min: values.iter().copied().fold(f64::INFINITY, f64::min),
				max: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
			})
		};
		let mut context_tiers: Vec<u32> = tiers.iter().map(|tier| tier.min_context_tokens).collect();
		context_tiers.sort_unstable();
		Some(PriceSummary {
			input: range(|rates| rates.input)?,
			output: range(|rates| rates.output)?,
			context_tiers,
			service_tiers: services.into_keys().collect(),
			windows: windows.len(),
		})
	}
}

fn valid_service_tier(name: &str) -> bool {
	!name.is_empty() && name.len() <= 32 && name.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-')
}

fn minutes(value: &str) -> u32 {
	let (hours, minutes) = value.split_once(':').unwrap_or_default();
	hours.parse::<u32>().unwrap_or_default() * 60 + minutes.parse::<u32>().unwrap_or_default()
}

fn weekday_index(day: PriceWeekday) -> u32 {
	match day {
		PriceWeekday::Mon => 0,
		PriceWeekday::Tue => 1,
		PriceWeekday::Wed => 2,
		PriceWeekday::Thu => 3,
		PriceWeekday::Fri => 4,
		PriceWeekday::Sat => 5,
		PriceWeekday::Sun => 6,
	}
}

fn week_spans(window: &PriceWindow) -> Vec<(u32, u32)> {
	let (start, end) = (minutes(&window.start), minutes(&window.end));
	let length = if end > start { end - start } else { end + DAY_MINUTES - start };
	let days: Vec<u32> = if window.days.is_empty() { (0..7).collect() } else { window.days.iter().copied().map(weekday_index).collect() };
	let mut spans = Vec::with_capacity(days.len() * 2);
	for day in days {
		let from = day * DAY_MINUTES + start;
		let to = from + length;
		if to <= WEEK_MINUTES {
			spans.push((from, to));
		} else {
			spans.push((from, WEEK_MINUTES));
			spans.push((0, to - WEEK_MINUTES));
		}
	}
	spans
}

fn validate_windows(windows: &[PriceWindow]) -> Result<(), PriceSheetError> {
	if let Some(index) = windows.iter().position(|window| minutes(&window.start) == minutes(&window.end)) {
		return Err(PriceSheetError::EmptyWindow(index));
	}
	let spans: Vec<Vec<(u32, u32)>> = windows.iter().map(week_spans).collect();
	for (first, window) in windows.iter().enumerate() {
		for second in first + 1..windows.len() {
			if !window.timezone.eq_ignore_ascii_case(&windows[second].timezone) {
				continue;
			}
			let overlaps = spans[first].iter().any(|a| spans[second].iter().any(|b| a.0 < b.1 && b.0 < a.1));
			if overlaps {
				return Err(PriceSheetError::OverlappingWindows(first, second));
			}
		}
	}
	Ok(())
}
