mod ecb;
mod repository;

pub use ecb::EcbRates;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::FromRow;

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
