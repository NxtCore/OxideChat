use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use std::collections::HashMap;
use std::str::FromStr;

// ============================================================================
// Response Types
// ============================================================================

/// The value of one unit of a currency in US dollars, from the ECB reference rates.
#[derive(Debug, Clone, FromRow, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ExchangeRate {
	pub currency: String,
	#[cfg_attr(test, ts(type = "string"))]
	pub usd_per_unit: Decimal,
	pub rate_date: NaiveDate,
	pub fetched_at: DateTime<Utc>,
}

// ============================================================================
// Internal Types
// ============================================================================

/// Rates parsed from one ECB daily reference file, as US dollars per unit.
#[derive(Debug, Clone, PartialEq)]
pub struct EcbRates {
	pub date: NaiveDate,
	pub usd_per_unit: Vec<(String, Decimal)>,
}

fn attribute<'a>(text: &'a str, name: &str) -> Option<&'a str> {
	let start = text.find(&format!("{name}='"))? + name.len() + 2;
	let end = text[start..].find('\'')? + start;
	Some(&text[start..end])
}

impl EcbRates {
	/// Parses the ECB daily reference file, whose rates are units per euro, into US dollars per unit.
	/// Returns `None` when the file has no date or no US dollar rate.
	#[must_use]
	pub fn parse(xml: &str) -> Option<Self> {
		let date = NaiveDate::parse_from_str(attribute(xml, "time")?, "%Y-%m-%d").ok()?;
		let per_euro: Vec<(String, Decimal)> = xml
			.split("<Cube ")
			.filter_map(|cube| Some((attribute(cube, "currency")?.to_string(), Decimal::from_str(attribute(cube, "rate")?).ok()?)))
			.filter(|(code, rate)| code.len() == 3 && *rate > Decimal::ZERO)
			.collect();
		let usd_per_euro = per_euro.iter().find(|(code, _)| code == "USD")?.1;
		let mut usd_per_unit: Vec<(String, Decimal)> = per_euro
			.iter()
			.filter(|(code, _)| code != "USD")
			.map(|(code, rate)| (code.clone(), (usd_per_euro / rate).round_dp(10)))
			.collect();
		usd_per_unit.push(("EUR".to_string(), usd_per_euro));
		usd_per_unit.push(("USD".to_string(), Decimal::ONE));
		usd_per_unit.sort_by(|a, b| a.0.cmp(&b.0));
		Some(Self { date, usd_per_unit })
	}
}

impl ExchangeRate {
	/// Lists every stored rate, ordered by currency code.
	///
	/// # Errors
	/// Returns a database error when the query fails.
	pub async fn list(pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, Self>("SELECT currency, usd_per_unit, rate_date, fetched_at FROM exchange_rates ORDER BY currency")
			.fetch_all(pool)
			.await
	}

	/// US dollars per unit for every stored currency.
	///
	/// # Errors
	/// Returns a database error when the query fails.
	pub async fn usd_per_unit_map(pool: &PgPool) -> Result<HashMap<String, Decimal>, sqlx::Error> {
		Ok(Self::list(pool).await?.into_iter().map(|rate| (rate.currency, rate.usd_per_unit)).collect())
	}

	/// Stores parsed rates in one transaction and returns `true` when any rate changed.
	///
	/// # Errors
	/// Returns a database error when the rates cannot be stored.
	pub async fn store(pool: &PgPool, rates: &EcbRates) -> Result<bool, sqlx::Error> {
		let mut tx = pool.begin().await?;
		let mut changed = false;
		for (currency, usd_per_unit) in &rates.usd_per_unit {
			let result = sqlx::query(
				r#"
				INSERT INTO exchange_rates (currency, usd_per_unit, rate_date, fetched_at)
				VALUES ($1, $2, $3, NOW())
				ON CONFLICT (currency) DO UPDATE
				SET usd_per_unit = EXCLUDED.usd_per_unit, rate_date = EXCLUDED.rate_date, fetched_at = NOW()
				WHERE exchange_rates.usd_per_unit <> EXCLUDED.usd_per_unit OR exchange_rates.rate_date <> EXCLUDED.rate_date
				"#,
			)
			.bind(currency)
			.bind(usd_per_unit)
			.bind(rates.date)
			.execute(&mut *tx)
			.await?;
			changed |= result.rows_affected() > 0;
		}
		tx.commit().await?;
		Ok(changed)
	}
}
