//! HTTP tool executor for REST API-based tools.

use async_trait::async_trait;
use reqwest::{Client, RequestBuilder};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

use super::executor::{ToolContext, ToolError, ToolExecutor};

/// HTTP source configuration
#[derive(Debug, Clone)]
pub struct HttpConfig {
	pub url: String,
	pub method: String,
	pub headers: HashMap<String, String>,
	pub body_template: Option<String>,
}

/// Executor for HTTP-based tools
pub struct HttpExecutor {
	name: String,
	config: HttpConfig,
	client: Client,
}

impl HttpExecutor {
	/// Create a new HTTP executor
	///
	/// # Arguments
	/// * `name` - Tool name for logging
	/// * `config` - HTTP configuration
	///
	/// # Errors
	/// Returns `ToolError::HttpError` if the client cannot be created
	pub fn new(name: String, config: HttpConfig) -> Result<Self, ToolError> {
		let client = Client::builder()
			.timeout(Duration::from_secs(30))
			.redirect(reqwest::redirect::Policy::none())
			.build()
			.map_err(|e| ToolError::HttpError(format!("Failed to create HTTP client: {e}")))?;

		Ok(Self { name, config, client })
	}

	/// Substitute settings values into a string template
	fn substitute_template(template: &str, settings: &Value, input: &Value) -> String {
		let mut result = template.to_string();

		if let Value::Object(settings_map) = settings {
			for (key, value) in settings_map {
				let placeholder = format!("{{{{settings.{key}}}}}");
				if let Value::String(s) = value {
					result = result.replace(&placeholder, s);
				}
			}
		}

		if let Value::Object(input_map) = input {
			for (key, value) in input_map {
				let placeholder = format!("{{{{input.{key}}}}}");
				if let Value::String(s) = value {
					result = result.replace(&placeholder, s);
				} else {
					result = result.replace(&placeholder, &value.to_string());
				}
			}
		}

		result
	}

	/// Resolve a placeholder spanning an entire string to its typed JSON value
	fn exact_placeholder(text: &str, settings: &Value, input: &Value) -> Option<Value> {
		let inner = text.strip_prefix("{{")?.strip_suffix("}}")?;
		let (scope, key) = inner.split_once('.')?;
		match scope {
			"input" => input.get(key).cloned(),
			"settings" => settings.get(key).cloned(),
			_ => None,
		}
	}

	/// Recursively substitute placeholders inside a parsed JSON value
	fn substitute_json(node: &Value, settings: &Value, input: &Value) -> Value {
		match node {
			Value::String(text) => match Self::exact_placeholder(text, settings, input) {
				Some(value) => value,
				None => Value::String(Self::substitute_template(text, settings, input)),
			},
			Value::Array(items) => Value::Array(items.iter().map(|item| Self::substitute_json(item, settings, input)).collect()),
			Value::Object(map) => Value::Object(map.iter().map(|(key, value)| (key.clone(), Self::substitute_json(value, settings, input))).collect()),
			other => other.clone(),
		}
	}

	/// Substitute placeholders in a body template, keeping JSON structure when the template parses as JSON
	fn substitute_body(template: &str, settings: &Value, input: &Value) -> String {
		match serde_json::from_str::<Value>(template) {
			Ok(parsed) => match serde_json::to_string(&Self::substitute_json(&parsed, settings, input)) {
				Ok(body) => body,
				Err(_) => Self::substitute_template(template, settings, input),
			},
			Err(_) => Self::substitute_template(template, settings, input),
		}
	}

	/// Parse a URL and require an http or https scheme
	fn parse_http_url(text: &str) -> Result<reqwest::Url, ToolError> {
		let parsed = reqwest::Url::parse(text).map_err(|e| ToolError::InvalidInput(format!("Invalid tool URL: {e}")))?;
		if !matches!(parsed.scheme(), "http" | "https") {
			return Err(ToolError::InvalidInput("Tool URL must use http or https".to_string()));
		}
		Ok(parsed)
	}

	/// Substitute the URL template and verify the result stays on the configured origin
	fn resolve_url(&self, settings: &Value, input: &Value) -> Result<String, ToolError> {
		let base_text = Self::substitute_template(&self.config.url, settings, &Value::Null);
		let base = Self::parse_http_url(&base_text)?;
		let url = Self::substitute_template(&self.config.url, settings, input);
		let parsed = Self::parse_http_url(&url)?;
		if parsed.origin() != base.origin() || parsed.username() != base.username() || parsed.password() != base.password() {
			return Err(ToolError::InvalidInput("Tool URL must stay on the configured host".to_string()));
		}
		Ok(url)
	}

	/// Apply configured headers with substitution, rejecting control characters in substituted values
	fn apply_headers(&self, request: RequestBuilder, settings: &Value, input: &Value) -> Result<RequestBuilder, ToolError> {
		let mut request = request;
		for (key, value) in &self.config.headers {
			let substituted = Self::substitute_template(value, settings, input);
			if substituted.chars().any(char::is_control) {
				return Err(ToolError::InvalidInput(format!("Header '{key}' value contains control characters")));
			}
			request = request.header(key.as_str(), substituted);
		}
		Ok(request)
	}
}

#[async_trait]
impl ToolExecutor for HttpExecutor {
	async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<Value, ToolError> {
		let url = self.resolve_url(&ctx.settings, &input)?;

		let request = match self.config.method.to_uppercase().as_str() {
			"GET" => self.client.get(&url),
			"POST" => self.client.post(&url),
			"PUT" => self.client.put(&url),
			"DELETE" => self.client.delete(&url),
			"PATCH" => self.client.patch(&url),
			method => return Err(ToolError::InvalidInput(format!("Unsupported HTTP method: {method}"))),
		};

		let request = self.apply_headers(request, &ctx.settings, &input)?;

		let request = if let Some(body_template) = &self.config.body_template {
			request.body(Self::substitute_body(body_template, &ctx.settings, &input))
		} else if matches!(self.config.method.to_uppercase().as_str(), "POST" | "PUT" | "PATCH") {
			request.json(&input)
		} else {
			request
		};

		let request = match ctx.timeout_ms {
			Some(timeout_ms) => request.timeout(Duration::from_millis(timeout_ms)),
			None => request,
		};

		let response = request.send().await.map_err(|e| ToolError::HttpError(format!("HTTP request failed: {e}")))?;

		let status = response.status();
		let body = response
			.text()
			.await
			.map_err(|e| ToolError::HttpError(format!("Failed to read response body: {e}")))?;

		if !status.is_success() {
			tracing::debug!(tool = %self.name, status = %status, body = %body, "HTTP tool upstream returned an error status");
			return Err(ToolError::HttpError(format!("HTTP {status}")));
		}

		match serde_json::from_str::<Value>(&body) {
			Ok(json) => Ok(json),
			Err(_) => Ok(Value::String(body)),
		}
	}
}
