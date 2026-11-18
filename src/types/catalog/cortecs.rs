use super::GatewayProviderOptionSyncInput;
use omniference::catalog::{CortecsModel, CortecsPricing};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;

/// `source_gateway` of catalog rows synced from Cortecs.
pub const CORTECS_GATEWAY: &str = "cortecs";

/// One provider option per upstream provider of a Cortecs model. Prices are converted to USD per 1M tokens
/// with `usd_per_unit` (left empty when the currency has no rate), latency from seconds to milliseconds and
/// uptime from a fraction to percent; limits the provider does not report come from the model.
#[must_use]
pub fn cortecs_options(model: &CortecsModel, usd_per_unit: &HashMap<String, Decimal>) -> Vec<GatewayProviderOptionSyncInput> {
	model
		.providers_details
		.iter()
		.map(|(slug, details)| {
			let metrics = details.metrics.as_ref();
			let pricing = details.pricing.as_ref().or(model.pricing.as_ref());
			let rate = pricing.and_then(|pricing| usd_rate(pricing, usd_per_unit));
			let price = |value: Option<f64>| value.zip(rate).map(|(value, rate)| value * rate);
			let tokens = |own: Option<u64>, model: Option<u64>| own.or(model).and_then(|value| i32::try_from(value).ok());
			GatewayProviderOptionSyncInput {
				provider_slug: Some(slug.to_ascii_lowercase()),
				provider_name: Some(slug.clone()),
				endpoint_name: None,
				status: Some(1.0),
				quantization: details.quantization.clone(),
				context_length: tokens(details.context_size, model.context_size),
				max_completion_tokens: tokens(details.max_output_tokens, model.max_output_tokens),
				latency: metrics.and_then(|metrics| metrics.latency).map(|seconds| seconds * 1000.0),
				throughput: metrics.and_then(|metrics| metrics.throughput),
				uptime: metrics.and_then(|metrics| metrics.uptime).map(|fraction| fraction * 100.0),
				price_input: price(pricing.and_then(|pricing| pricing.input_token)),
				price_output: price(pricing.and_then(|pricing| pricing.output_token)),
				raw: serde_json::to_value(details).unwrap_or_default(),
			}
		})
		.collect()
}

fn usd_rate(pricing: &CortecsPricing, usd_per_unit: &HashMap<String, Decimal>) -> Option<f64> {
	let currency = pricing.currency.as_deref()?.to_ascii_uppercase();
	if currency == "USD" {
		return Some(1.0);
	}
	usd_per_unit.get(&currency).and_then(ToPrimitive::to_f64)
}
