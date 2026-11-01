use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
pub struct PageQuery {
	pub page: Option<i64>,
	pub size: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Page<T> {
	pub has_more: bool,
	pub items: Vec<T>,
}
