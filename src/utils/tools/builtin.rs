//! Built-in tool executors (Exa search, etc.)

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

use super::executor::{ToolContext, ToolError, ToolExecutor};

/// Get a builtin executor by ID
///
/// # Arguments
/// * `builtin_id` - The builtin tool identifier (e.g., "exa_search")
///
/// # Errors
/// Returns `ToolError::NotFound` if the builtin ID is unknown
pub fn get_builtin_executor(builtin_id: &str) -> Result<Box<dyn ToolExecutor>, ToolError> {
	match builtin_id {
		"exa_search" => Ok(Box::new(ExaSearchExecutor::new()?)),
		_ => Err(ToolError::NotFound(format!("Unknown builtin tool: {builtin_id}"))),
	}
}

// ============= Exa Search =============

const EXA_API_URL: &str = "https://api.exa.ai/search";

/// Exa search request
#[derive(Debug, Serialize)]
struct ExaSearchRequest {
	query: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	num_results: Option<u32>,
	#[serde(rename = "type", skip_serializing_if = "Option::is_none")]
	search_type: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	use_autoprompt: Option<bool>,
	contents: ExaContentsConfig,
}

#[derive(Debug, Serialize)]
struct ExaContentsConfig {
	text: bool,
	highlights: bool,
}

/// Exa search result
#[derive(Debug, Deserialize)]
struct ExaSearchResponse {
	results: Vec<ExaResult>,
}

#[derive(Debug, Deserialize)]
struct ExaResult {
	title: Option<String>,
	url: String,
	text: Option<String>,
	highlights: Option<Vec<String>>,
	score: Option<f64>,
}

/// Executor for Exa AI search
pub struct ExaSearchExecutor {
	client: Client,
}

impl ExaSearchExecutor {
	/// Create a new Exa search executor
	pub fn new() -> Result<Self, ToolError> {
		let client = Client::builder()
			.timeout(Duration::from_secs(30))
			.build()
			.map_err(|e| ToolError::Internal(format!("Failed to create HTTP client: {e}")))?;

		Ok(Self { client })
	}
}

#[async_trait]
impl ToolExecutor for ExaSearchExecutor {
	async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<Value, ToolError> {
		// Get API key from user settings
		let api_key = ctx
			.settings
			.get("api_key")
			.and_then(Value::as_str)
			.ok_or_else(|| ToolError::MissingSetting("api_key".to_string()))?;

		// Parse input
		let query = input
			.get("query")
			.and_then(Value::as_str)
			.ok_or_else(|| ToolError::InvalidInput("Missing required field: query".to_string()))?;

		let num_results = input.get("num_results").and_then(Value::as_u64).map(|n| n as u32);

		let search_type = input.get("type").and_then(Value::as_str).map(String::from);

		// Build request
		let request_body = ExaSearchRequest {
			query: query.to_string(),
			num_results,
			search_type,
			use_autoprompt: Some(true),
			contents: ExaContentsConfig { text: true, highlights: true },
		};

		// Make API call
		let response = self
			.client
			.post(EXA_API_URL)
			.header("Authorization", format!("Bearer {api_key}"))
			.header("Content-Type", "application/json")
			.json(&request_body)
			.send()
			.await
			.map_err(|e| ToolError::HttpError(format!("Exa API request failed: {e}")))?;

		let status = response.status();
		if !status.is_success() {
			let error_body = response.text().await.unwrap_or_default();
			return Err(ToolError::HttpError(format!("Exa API error {status}: {error_body}")));
		}

		let exa_response: ExaSearchResponse = response
			.json()
			.await
			.map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse Exa response: {e}")))?;

		// Convert to output format
		let results: Vec<Value> = exa_response
			.results
			.iter()
			.map(|r| {
				json!({
					"title": r.title,
					"url": r.url,
					"text": r.text,
					"highlights": r.highlights,
					"score": r.score
				})
			})
			.collect();

		Ok(json!({
			"results": results,
			"count": results.len()
		}))
	}

	fn name(&self) -> &str {
		"exa_search"
	}
}

/// Get the Exa search tool input schema
#[must_use]
pub fn exa_search_input_schema() -> Value {
	json!({
		"type": "object",
		"properties": {
			"query": {
				"type": "string",
				"description": "The search query"
			},
			"num_results": {
				"type": "integer",
				"description": "Number of results to return (default: 10)",
				"default": 10
			},
			"type": {
				"type": "string",
				"enum": ["neural", "keyword", "auto"],
				"description": "Search type (default: auto)",
				"default": "auto"
			}
		},
		"required": ["query"]
	})
}

/// Get the Exa search tool settings schema
#[must_use]
pub fn exa_search_settings_schema() -> Value {
	json!({
		"type": "object",
		"properties": {
			"api_key": {
				"type": "string",
				"title": "Exa API Key",
				"description": "Your Exa AI API key from https://exa.ai",
				"secret": true
			}
		},
		"required": ["api_key"]
	})
}

// ============= Builtin Executor wrapper =============

/// Generic wrapper for builtin executors
pub struct BuiltinExecutor {
	inner: Box<dyn ToolExecutor>,
}

impl BuiltinExecutor {
	/// Create a builtin executor from a builtin ID
	pub fn new(builtin_id: &str) -> Result<Self, ToolError> {
		let inner = get_builtin_executor(builtin_id)?;
		Ok(Self { inner })
	}
}

#[async_trait]
impl ToolExecutor for BuiltinExecutor {
	async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<Value, ToolError> {
		self.inner.execute(input, ctx).await
	}

	fn name(&self) -> &str {
		self.inner.name()
	}
}
