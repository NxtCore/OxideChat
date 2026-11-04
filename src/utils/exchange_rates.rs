use crate::types::exchange_rates::{EcbRates, ExchangeRate};
use sqlx::PgPool;
use std::time::Duration;

/// The European Central Bank's daily euro reference rates (no key needed, updated on banking days).
pub const ECB_DAILY_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";

/// Fetches the ECB reference rates and stores them as US dollars per unit.
/// Returns `true` when any stored rate changed.
///
/// # Errors
/// Returns a message when the file cannot be fetched, parsed or stored.
pub async fn refresh_exchange_rates(pool: &PgPool) -> Result<bool, String> {
	let xml = omniference::adapter::shared_http_client()
		.get(ECB_DAILY_URL)
		.timeout(Duration::from_secs(20))
		.send()
		.await
		.and_then(reqwest::Response::error_for_status)
		.map_err(|error| format!("ECB rates request failed: {error}"))?
		.text()
		.await
		.map_err(|error| format!("ECB rates could not be read: {error}"))?;
	let rates = EcbRates::parse(&xml).ok_or_else(|| "ECB rates file has no date or no US dollar rate".to_string())?;
	ExchangeRate::store(pool, &rates).await.map_err(|error| format!("ECB rates could not be stored: {error}"))
}
