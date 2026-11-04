#[cfg(test)]
mod parsing {
	use crate::types::exchange_rates::EcbRates;
	use rust_decimal::Decimal;
	use std::str::FromStr;

	pub(crate) const ECB_SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
	<gesmes:subject>Reference rates</gesmes:subject>
	<gesmes:Sender><gesmes:name>European Central Bank</gesmes:name></gesmes:Sender>
	<Cube>
		<Cube time='2026-11-04'>
			<Cube currency='USD' rate='1.0800'/>
			<Cube currency='JPY' rate='162.00'/>
			<Cube currency='GBP' rate='0.8400'/>
		</Cube>
	</Cube>
</gesmes:Envelope>"#;

	fn rate(rates: &EcbRates, code: &str) -> Option<Decimal> {
		rates.usd_per_unit.iter().find(|(currency, _)| currency == code).map(|(_, rate)| *rate)
	}

	#[test]
	fn rates_per_euro_become_us_dollars_per_unit() {
		let rates = EcbRates::parse(ECB_SAMPLE).unwrap_or_else(|| panic!("sample did not parse"));
		assert_eq!(rates.date.to_string(), "2026-11-04");
		assert_eq!(rate(&rates, "EUR"), Decimal::from_str("1.0800").ok());
		assert_eq!(rate(&rates, "USD"), Some(Decimal::ONE));
		assert_eq!(rate(&rates, "GBP"), Decimal::from_str("1.2857142857").ok());
		assert_eq!(rate(&rates, "JPY"), Decimal::from_str("0.0066666667").ok());
		assert_eq!(rates.usd_per_unit.len(), 4);
	}

	#[test]
	fn a_file_without_a_date_or_dollar_rate_is_rejected() {
		assert!(EcbRates::parse(&ECB_SAMPLE.replace("time='2026-11-04'", "")).is_none());
		assert!(EcbRates::parse(&ECB_SAMPLE.replace("currency='USD'", "currency='CHF'")).is_none());
		assert!(EcbRates::parse("<html>maintenance</html>").is_none());
	}
}

#[cfg(test)]
mod storage {
	use super::parsing::ECB_SAMPLE;
	use crate::types::exchange_rates::{EcbRates, ExchangeRate};
	use rust_decimal::Decimal;
	use sqlx::PgPool;
	use std::str::FromStr;

	type TestResult = Result<(), Box<dyn std::error::Error>>;

	#[sqlx::test(migrations = "./migrations")]
	async fn storing_reports_whether_any_rate_changed(pool: PgPool) -> TestResult {
		let rates = EcbRates::parse(ECB_SAMPLE).ok_or("sample did not parse")?;
		assert!(ExchangeRate::store(&pool, &rates).await?);
		assert!(!ExchangeRate::store(&pool, &rates).await?);

		let map = ExchangeRate::usd_per_unit_map(&pool).await?;
		assert_eq!(map.get("EUR"), Decimal::from_str("1.08").ok().as_ref());

		let moved = EcbRates::parse(&ECB_SAMPLE.replace("rate='1.0800'", "rate='1.0900'")).ok_or("sample did not parse")?;
		assert!(ExchangeRate::store(&pool, &moved).await?);
		let listed = ExchangeRate::list(&pool).await?;
		assert_eq!(listed.iter().map(|rate| rate.currency.as_str()).collect::<Vec<_>>(), ["EUR", "GBP", "JPY", "USD"]);
		Ok(())
	}
}
