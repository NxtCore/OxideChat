//! Shared types for OxideChat API.
//!
//! This module contains all request/response DTOs and domain types.

pub trait BaseType {
	const DEFAULT_PAGE_SIZE: i64 = 50;
	const MAX_PAGE_SIZE: i64 = 100;

	const TABLE: &'static str;
	const ALIAS: &'static str;

	fn new() -> Self;
	fn sql_fields() -> &'static [&'static str];

	fn aliased_fields() -> Result<Vec<String>, sqlx::Error> {
		Self::aliased_fields_from_list(Self::sql_fields())
	}

	fn aliased_fields_from_list(fields: &[&str]) -> Result<Vec<String>, sqlx::Error> {
		let sql_fields = Self::sql_fields();
		if let Some(invalid_field) = fields.iter().find(|f| !sql_fields.contains(*f)) {
			return Err(sqlx::Error::ColumnNotFound(format!("{}.{}", Self::TABLE, invalid_field)));
		}

		Ok(fields.iter().map(|f| format!("{}.{} AS {}_{}", Self::ALIAS, f, Self::ALIAS, f)).collect())
	}

	fn aliased_fields_str() -> Result<String, sqlx::Error> {
		Ok(Self::aliased_fields()?.join(", "))
	}

	fn aliased_fields_str_from_list(fields: &[&str]) -> Result<String, sqlx::Error> {
		Ok(Self::aliased_fields_from_list(fields)?.join(", "))
	}

	fn pagination(page: i64, size: i64) -> Pagination {
		let page_size = if size <= 0 { Self::DEFAULT_PAGE_SIZE } else { size.min(Self::MAX_PAGE_SIZE) };
		let page_size = page_size.max(1);

		Pagination {
			offset: (page.max(1) - 1) * page_size,
			limit: page_size + 1,
			page_size: page_size as usize,
		}
	}
}

pub struct Pagination {
	pub offset: i64,
	pub limit: i64,
	pub page_size: usize,
}

pub mod ai;
pub mod auth;
pub mod base;
pub mod chat;
pub mod i18n;
pub mod images;
pub mod logging;
pub mod oauth;
pub mod permissions;
pub mod roles;
pub mod state;
pub mod tools;
pub mod user;
pub mod models;
pub mod providers;
pub mod models_configs;
pub mod axum;

pub use ai::*;
pub use auth::*;
pub use base::*;
pub use chat::*;
pub use i18n::*;
pub use images::*;
pub use permissions::*;
pub use roles::*;
pub use state::*;
pub use tools::*;
pub use user::*;
