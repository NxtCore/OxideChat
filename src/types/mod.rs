//! Shared types for OxideChat API.
//!
//! This module contains all request/response DTOs and domain types.

pub trait BaseType {
	const DEFAULT_PAGE_SIZE: i64 = 50;
	const MAX_PAGE_SIZE: i64 = 100;

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

pub mod auth;
pub mod axum;
pub mod base;
pub mod catalog;
pub mod chat;
pub mod i18n;
pub mod images;
pub mod models;
pub mod models_configs;
pub mod oauth;
pub mod permissions;
pub mod providers;
pub mod roles;
pub mod state;
pub mod tools;
pub mod user;

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
