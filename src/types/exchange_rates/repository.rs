use super::{EcbRates, ExchangeRate};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;

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
