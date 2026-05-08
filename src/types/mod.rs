//! Shared types for OxideChat API.
//!
//! This module contains all request/response DTOs and domain types.

pub trait BaseType {
    const TABLE: &'static str;
    const ALIAS: &'static str;

    fn new() -> Self;
    fn sql_fields() -> &'static [&'static str];

    fn aliased_fields() -> Vec<String> {
        Self::sql_fields()
            .iter()
            .map(|f| format!("{}.{} AS {}_{}", Self::ALIAS, f, Self::ALIAS, f))
            .collect()
    }

    fn aliased_fields_from_list(fields: &[&str]) -> Vec<String> {
        let sql_fields = Self::sql_fields();
        let invalid_fields: Vec<String> = fields
            .iter()
            .filter(|f| !sql_fields.contains(*f))
            .map(|f| f.to_string())
            .collect();

        if !invalid_fields.is_empty() {
            panic!("Invalid fields for {}: {:?}", Self::TABLE, invalid_fields);
        }

        fields
            .iter()
            .map(|f| format!("{}.{} AS {}_{}", Self::ALIAS, f, Self::ALIAS, f))
            .collect()
    }

    fn aliased_fields_str() -> String {
        Self::aliased_fields().join(", ")
    }

    fn aliased_fields_str_from_list(fields: &[&str]) -> String {
        Self::aliased_fields_from_list(fields).join(", ")
    }
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