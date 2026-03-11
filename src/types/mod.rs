//! Shared types for OxideChat API.
//!
//! This module contains all request/response DTOs and domain types.

pub trait BaseType {
    fn new() -> Self;
    fn table(&self) -> &str;
    fn alias(&self) -> &str;
    fn sql_fields(&self) -> Vec<&str>;
    fn aliased_fields(&self) -> Vec<String> {
        self.sql_fields()
            .iter()
            .map(|f| format!("{}.{} AS {}_{}", self.alias(), f, self.alias(), f))
            .collect()
    }
    fn aliased_fields_from_list(&self, fields: Vec<&str>) -> Vec<String> {
        let invalid_fields: Vec<String> = fields
            .iter()
            .filter(|f| !self.sql_fields().contains(*f))
            .map(|f| f.to_string())
            .collect();

        if !invalid_fields.is_empty() {
            panic!("Invalid fields for {}: {:?}", self.table(), invalid_fields);
        }

        fields
            .iter()
            .map(|f| format!("{}.{} AS {}_{}", self.alias(), f, self.alias(), f))
            .collect()
    }
    fn aliased_fields_str(&self) -> String {
        self.aliased_fields().join(", ")
    }
    fn aliased_fields_str_from_list(&self, fields: Vec<&str>) -> String {
        self.aliased_fields_from_list(fields).join(", ")
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