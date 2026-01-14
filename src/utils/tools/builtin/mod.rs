//! Built-in tool implementations organized by category

pub mod imagegen;
pub mod websearch;

use super::executor::{ToolError, ToolExecutor};

/// Get a builtin executor by ID
///
/// # Arguments
/// * `builtin_id` - The builtin tool identifier (e.g., "websearch", "imagegen")
///
/// # Errors
/// Returns `ToolError::NotFound` if the builtin ID is unknown
pub fn get_builtin_executor(builtin_id: &str) -> Result<Box<dyn ToolExecutor>, ToolError> {
	match builtin_id {
		"websearch" => Ok(Box::new(websearch::WebsearchExecutor::new()?)),
		"imagegen" => Ok(Box::new(imagegen::ImageGenExecutor::new()?)),
		_ => Err(ToolError::NotFound(format!("Unknown builtin tool: {builtin_id}"))),
	}
}
