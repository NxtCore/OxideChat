//! Tool execution infrastructure.
//!
//! This module provides the executor abstraction and implementations for
//! different tool source types (WASM, MCP, HTTP, Builtin).

mod builtin;
mod executor;
pub mod http;
pub mod mcp;
mod wasm;

pub use builtin::{BuiltinExecutor, get_builtin_executor};
pub use executor::{ToolContext, ToolError, ToolExecutor};
pub use http::HttpExecutor;
pub use mcp::{McpClient, McpExecutor};
pub use wasm::WasmExecutor;
