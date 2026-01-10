//! HTTP tool executor for REST API-based tools.

use async_trait::async_trait;
use reqwest::Client;
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
}

#[async_trait]
impl ToolExecutor for HttpExecutor {
	async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<Value, ToolError> {
		let url = Self::substitute_template(&self.config.url, &ctx.settings, &input);

		let mut request = match self.config.method.to_uppercase().as_str() {
			"GET" => self.client.get(&url),
			"POST" => self.client.post(&url),
			"PUT" => self.client.put(&url),
			"DELETE" => self.client.delete(&url),
			"PATCH" => self.client.patch(&url),
			method => return Err(ToolError::InvalidInput(format!("Unsupported HTTP method: {method}"))),
		};

		for (key, value) in &self.config.headers {
			let substituted_value = Self::substitute_template(value, &ctx.settings, &input);
			request = request.header(key.as_str(), substituted_value);
		}

		if let Some(body_template) = &self.config.body_template {
			let body = Self::substitute_template(body_template, &ctx.settings, &input);
			request = request.body(body);
		} else if matches!(self.config.method.to_uppercase().as_str(), "POST" | "PUT" | "PATCH") {
			request = request.json(&input);
		}

		if let Some(timeout_ms) = ctx.timeout_ms {
			request = request.timeout(Duration::from_millis(timeout_ms));
		}

		let response = request.send().await.map_err(|e| ToolError::HttpError(format!("HTTP request failed: {e}")))?;

		let status = response.status();
		let body = response
			.text()
			.await
			.map_err(|e| ToolError::HttpError(format!("Failed to read response body: {e}")))?;

		if !status.is_success() {
			return Err(ToolError::HttpError(format!("HTTP {status}: {body}")));
		}

		match serde_json::from_str::<Value>(&body) {
			Ok(json) => Ok(json),
			Err(_) => Ok(Value::String(body)),
		}
	}

	fn name(&self) -> &str {
		&self.name
	}
}
