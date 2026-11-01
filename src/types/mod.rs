//! Shared types for `OxideChat` API.
//!
//! This module contains all request/response DTOs and domain types.

pub trait BaseType {
	const DEFAULT_PAGE_SIZE: i64 = 50;
	const MAX_PAGE_SIZE: i64 = 100;

	fn pagination(query: &PageQuery) -> Pagination {
		let size = query.size.unwrap_or(0);
		let page_size = if size <= 0 { Self::DEFAULT_PAGE_SIZE } else { size.min(Self::MAX_PAGE_SIZE) };
		let page_size = page_size.max(1);

		Pagination {
			offset: (query.page.unwrap_or(1).max(1) - 1) * page_size,
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

pub mod auth;
pub mod auth_user;
pub mod base;
pub mod budgets;
pub mod catalog;
pub mod chat;
pub mod error;
pub mod gateway;
pub mod i18n;
pub mod images;
pub mod models;
pub mod models_configs;
pub mod oauth;
pub mod page;
pub mod permissions;
pub mod policy;
pub mod providers;
pub mod request_logs;
pub mod roles;
pub mod state;
pub mod teams;
pub mod tools;
pub mod usage;
pub mod user;

pub use auth::*;
pub use auth_user::*;
pub use base::*;
pub use budgets::*;
pub use chat::*;
pub use error::*;
pub use gateway::*;
pub use i18n::*;
pub use images::*;
pub use page::*;
pub use permissions::*;
pub use policy::*;
pub use request_logs::*;
pub use roles::*;
pub use state::*;
pub use teams::*;
pub use tools::*;
pub use usage::*;
pub use user::*;
