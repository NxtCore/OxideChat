#[cfg(test)]
mod reported_cost {
	use crate::types::providers::{ProviderReportedCost, ReportedCostUnit};
	use omniference::reported_cost::{CostSource, CostUnit};
	use rust_decimal::Decimal;
	use serde_json::json;
	use std::collections::HashMap;
	use std::str::FromStr;

	fn cost(value: serde_json::Value) -> ProviderReportedCost {
		serde_json::from_value(value).unwrap_or(ProviderReportedCost {
			unit: ReportedCostUnit::Usd,
			currency: None,
			usd_per_credit: None,
			source: Some("invalid source".to_string()),
		})
	}

	#[test]
	fn each_unit_requires_what_it_converts_with() {
		assert!(cost(json!({"unit": "usd"})).is_valid());
		assert!(cost(json!({"unit": "currency", "currency": "EUR"})).is_valid());
		assert!(!cost(json!({"unit": "currency"})).is_valid());
		assert!(!cost(json!({"unit": "currency", "currency": "eur"})).is_valid());
		assert!(!cost(json!({"unit": "currency", "currency": "EURO"})).is_valid());
		assert!(cost(json!({"unit": "credits", "usd_per_credit": "0.0001"})).is_valid());
		assert!(!cost(json!({"unit": "credits", "usd_per_credit": "0"})).is_valid());
		assert!(!cost(json!({"unit": "credits"})).is_valid());
	}

	#[test]
	fn sources_are_body_paths_or_headers() {
		for source in ["usage.cost", "usage.cost.total_cost", "x_nanogpt_pricing.cost", "header:x-litellm-response-cost"] {
			assert!(cost(json!({"unit": "usd", "source": source})).is_valid(), "{source}");
		}
		for source in ["", "usage..cost", ".cost", "header:", "header:x cost", "usage.cost[0]", &"a".repeat(201)] {
			assert!(!cost(json!({"unit": "usd", "source": source})).is_valid(), "{source}");
		}
	}

	#[test]
	fn the_omniference_config_converts_units_and_sources() {
		let rates = HashMap::from([("EUR".to_string(), Decimal::from_str("1.08").unwrap_or_default())]);

		let eur = cost(json!({"unit": "currency", "currency": "EUR", "source": "header:x-cost"})).to_omniference(&rates);
		let eur = eur.unwrap_or_else(|| panic!("EUR has a rate"));
		assert_eq!(eur.unit, CostUnit::Currency { code: "EUR".to_string(), usd_per_unit: 1.08 });
		assert_eq!(eur.source, Some(CostSource::Header { name: "x-cost".to_string() }));

		let credits = cost(json!({"unit": "credits", "usd_per_credit": "0.0001", "source": "usage.cost"})).to_omniference(&rates);
		let credits = credits.unwrap_or_else(|| panic!("credits have a value"));
		assert_eq!(credits.unit, CostUnit::Credits { usd_per_credit: 0.0001 });
		assert_eq!(credits.source, Some(CostSource::Body { path: "usage.cost".to_string() }));

		let usd = cost(json!({"unit": "usd"})).to_omniference(&rates).unwrap_or_else(|| panic!("usd needs nothing"));
		assert_eq!(usd.source, None);
		assert!(cost(json!({"unit": "currency", "currency": "CHF"})).to_omniference(&rates).is_none());
	}
}

#[cfg(test)]
mod storage {
	use crate::tests::gateway::support::{TestResult, create_provider_model};
	use crate::types::providers::{Provider, ProviderReportedCost, ProviderResponse};
	use serde_json::json;
	use sqlx::PgPool;

	#[sqlx::test(migrations = "./migrations")]
	async fn reported_cost_is_saved_returned_and_cleared(pool: PgPool) -> TestResult {
		let (provider_id, _) = create_provider_model(&pool, "Cortecs", "cortecs-model").await?;
		assert!(!Provider::any_reports_cost_in_currency(&pool).await?);

		let config: ProviderReportedCost = serde_json::from_value(json!({"unit": "currency", "currency": "EUR"}))?;
		let saved = Provider::set_reported_cost(&pool, &provider_id, Some(&config)).await?.ok_or("provider missing")?;
		let response: ProviderResponse = saved.into();
		assert_eq!(response.reported_cost, Some(config));
		assert!(Provider::any_reports_cost_in_currency(&pool).await?);
		let reloaded = Provider::find_for_admin(&pool, &provider_id).await?.ok_or("provider missing")?;
		assert!(reloaded.reported_cost.is_some());

		let cleared = Provider::set_reported_cost(&pool, &provider_id, None).await?.ok_or("provider missing")?;
		assert!(cleared.reported_cost.is_none());
		assert!(!Provider::any_reports_cost_in_currency(&pool).await?);
		Ok(())
	}
}
