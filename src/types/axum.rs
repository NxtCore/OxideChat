use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PaginatedResponse<T> {
	pub has_more: bool,
	pub items: Vec<T>,
}
