use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::str::FromStr;

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

