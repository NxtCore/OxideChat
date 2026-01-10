use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

use super::super::executor::{ToolContext, ToolError, ToolExecutor};

const EXA_API_URL: &str = "https://api.exa.ai/search";
const EXA_CONTENTS_URL: &str = "https://api.exa.ai/contents";
const TAVILY_API_URL: &str = "https://api.tavily.com/search";
const TAVILY_EXTRACT_URL: &str = "https://api.tavily.com/extract";

#[derive(Debug, Serialize, Deserialize)]
pub struct WebsearchInput {
	pub query: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub num_results: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CrawlInput {
	pub url: String,
}

#[derive(Debug, Serialize)]
pub struct WebsearchResult {
	pub title: Option<String>,
	pub url: String,
	pub description: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExaSearchRequest {
	query: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	num_results: Option<u32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	use_autoprompt: Option<bool>,
	contents: ExaContentsConfig,
}

#[derive(Debug, Serialize)]
struct ExaContentsConfig {
	text: bool,
	highlights: bool,
}

#[derive(Debug, Deserialize)]
struct ExaSearchResponse {
	results: Vec<ExaResult>,
}

#[derive(Debug, Deserialize)]
struct ExaResult {
	title: Option<String>,
	url: String,
	text: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExaContentsRequest {
	ids: Vec<String>,
	text: bool,
}

#[derive(Debug, Deserialize)]
struct ExaContentsResponse {
	results: Vec<ExaContentResult>,
}

#[derive(Debug, Deserialize)]
struct ExaContentResult {
	url: String,
	title: Option<String>,
	text: String,
}

#[derive(Debug, Serialize)]
struct TavilySearchRequest {
	#[serde(rename = "api_key")]
	api_key: String,
	query: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	max_results: Option<u32>,
	include_answer: bool,
	include_raw_content: bool,
}

#[derive(Debug, Deserialize)]
struct TavilySearchResponse {
	results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
	title: Option<String>,
	url: String,
	content: Option<String>,
}

#[derive(Debug, Serialize)]
struct TavilyExtractRequest {
	#[serde(rename = "api_key")]
	api_key: String,
	urls: Vec<String>,
	include_images: bool,
}

#[derive(Debug, Deserialize)]
struct TavilyExtractResponse {
	results: Vec<TavilyExtractResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyExtractResult {
	url: String,
	raw_content: Option<String>,
}

pub struct WebsearchExecutor {
	client: Client,
}

impl WebsearchExecutor {
	pub fn new() -> Result<Self, ToolError> {
		let client = Client::builder()
			.timeout(Duration::from_secs(30))
			.build()
			.map_err(|e| ToolError::Internal(format!("Failed to create HTTP client: {e}")))?;

		Ok(Self { client })
	}

	async fn execute_exa(&self, input: &WebsearchInput, api_key: &str) -> Result<Value, ToolError> {
		let request_body = ExaSearchRequest {
			query: input.query.clone(),
			num_results: input.num_results,
			use_autoprompt: Some(true),
			contents: ExaContentsConfig { text: true, highlights: true },
		};

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

		let results: Vec<WebsearchResult> = exa_response
			.results
			.into_iter()
			.map(|r| WebsearchResult {
				title: r.title,
				url: r.url,
				description: r.text,
			})
			.collect();

		Ok(json!({
			"results": results,
			"count": results.len(),
			"provider": "exa"
		}))
	}

	async fn execute_crawl_exa(&self, input: &CrawlInput, api_key: &str) -> Result<Value, ToolError> {
		let request_body = ExaContentsRequest {
			ids: vec![input.url.clone()],
			text: true,
		};

		let response = self
			.client
			.post(EXA_CONTENTS_URL)
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

		let exa_response: ExaContentsResponse = response
			.json()
			.await
			.map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse Exa response: {e}")))?;

		let results: Vec<WebsearchResult> = exa_response
			.results
			.into_iter()
			.map(|r| WebsearchResult {
				title: r.title,
				url: r.url,
				description: Some(r.text),
			})
			.collect();

		Ok(json!({
			"results": results,
			"count": results.len(),
			"provider": "exa",
			"mode": "crawl"
		}))
	}

	async fn execute_tavily(&self, input: &WebsearchInput, api_key: &str) -> Result<Value, ToolError> {
		let request_body = TavilySearchRequest {
			api_key: api_key.to_string(),
			query: input.query.clone(),
			max_results: input.num_results,
			include_answer: true,
			include_raw_content: true,
		};

		let response = self
			.client
			.post(TAVILY_API_URL)
			.header("Content-Type", "application/json")
			.json(&request_body)
			.send()
			.await
			.map_err(|e| ToolError::HttpError(format!("Tavily API request failed: {e}")))?;

		let status = response.status();
		if !status.is_success() {
			let error_body = response.text().await.unwrap_or_default();
			return Err(ToolError::HttpError(format!("Tavily API error {status}: {error_body}")));
		}

		let tavily_response: TavilySearchResponse = response
			.json()
			.await
			.map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse Tavily response: {e}")))?;

		let results: Vec<WebsearchResult> = tavily_response
			.results
			.into_iter()
			.map(|r| WebsearchResult {
				title: r.title,
				url: r.url,
				description: r.content,
			})
			.collect();

		Ok(json!({
			"results": results,
			"count": results.len(),
			"provider": "tavily"
		}))
	}
	async fn execute_crawl_tavily(&self, input: &CrawlInput, api_key: &str) -> Result<Value, ToolError> {
		let request_body = TavilyExtractRequest {
			api_key: api_key.to_string(),
			urls: vec![input.url.clone()],
			include_images: false,
		};

		let response = self
			.client
			.post(TAVILY_EXTRACT_URL)
			.header("Content-Type", "application/json")
			.json(&request_body)
			.send()
			.await
			.map_err(|e| ToolError::HttpError(format!("Tavily Extract API request failed: {e}")))?;

		let status = response.status();
		if !status.is_success() {
			let error_body = response.text().await.unwrap_or_default();
			return Err(ToolError::HttpError(format!("Tavily Extract API error {status}: {error_body}")));
		}

		let tavily_response: TavilyExtractResponse = response
			.json()
			.await
			.map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse Tavily Extract response: {e}")))?;

		let results: Vec<WebsearchResult> = tavily_response
			.results
			.into_iter()
			.map(|r| WebsearchResult {
				title: None,
				url: r.url,
				description: r.raw_content,
			})
			.collect();

		Ok(json!({
			"results": results,
			"count": results.len(),
			"provider": "tavily",
			"mode": "crawl"
		}))
	}
}

#[async_trait]
impl ToolExecutor for WebsearchExecutor {
	async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<Value, ToolError> {
		let function = match ctx.function_name.as_deref() {
			Some("search") | Some("websearch") | None => "search",
			Some("crawl") => "crawl",
			Some(other) => return Err(ToolError::ExecutionFailed(format!("Unknown function: {}", other))),
		};

		let api_key = ctx
			.settings
			.get("api_key")
			.and_then(Value::as_str)
			.ok_or_else(|| ToolError::MissingSetting("api_key".to_string()))?;

		let provider = ctx.settings.get("provider").and_then(Value::as_str).unwrap_or("exa");

		match (function, provider) {
			("crawl", "tavily") => {
				let crawl_input: CrawlInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.execute_crawl_tavily(&crawl_input, api_key).await
			}
			("crawl", _) => {
				let crawl_input: CrawlInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.execute_crawl_exa(&crawl_input, api_key).await
			}
			(_, "tavily") => {
				let search_input: WebsearchInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.execute_tavily(&search_input, api_key).await
			}
			_ => {
				let search_input: WebsearchInput = serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(format!("Invalid input: {e}")))?;
				self.execute_exa(&search_input, api_key).await
			}
		}
	}

	fn name(&self) -> &str {
		"websearch"
	}
}

/// Get the websearch tool input schema
#[must_use]
pub fn input_schema() -> Value {
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
				"default": 10,
				"minimum": 1,
				"maximum": 20
			}
		},
		"required": ["query"]
	})
}

/// Get the websearch tool settings schema
#[must_use]
pub fn settings_schema() -> Value {
	json!({
		"type": "object",
		"required": ["api_key", "provider"],
		"properties": {
			"api_key": {
				"type": "string",
				"secret": true,
				"description": "Get your API key from the specified provider"
			},
			"provider": {
				"type": "string",
				"enum": ["exa", "tavily"],
				"description": "Web search provider to use"
			}
		}
	})
}
