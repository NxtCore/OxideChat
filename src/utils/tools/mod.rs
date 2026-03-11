//! Tool execution infrastructure.
//!
//! This module provides the executor abstraction and implementations for
//! different tool source types (WASM, MCP, HTTP, Builtin).

mod builtin;
mod executor;
pub mod http;
pub mod mcp;
mod wasm;

pub use builtin::get_builtin_executor;
pub use executor::{ToolContext, ToolExecutor};
pub use http::HttpExecutor;
pub use wasm::WasmExecutor;
