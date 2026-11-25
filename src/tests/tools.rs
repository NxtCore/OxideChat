#[cfg(test)]
mod tests {
	use std::collections::HashMap;
	use std::sync::Arc;
	use std::sync::atomic::{AtomicUsize, Ordering};

	use serde_json::{Value, json};
	use tokio::io::{AsyncReadExt, AsyncWriteExt};
	use tokio::net::{TcpListener, TcpStream};

	use crate::utils::tools::http::{HttpConfig, HttpExecutor};
	use crate::utils::tools::{ToolContext, ToolError, ToolExecutor};

	fn http_config(url: &str, body_template: Option<&str>, headers: &[(&str, &str)]) -> HttpConfig {
		HttpConfig {
			url: url.to_string(),
			method: "POST".to_string(),
			headers: headers.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect::<HashMap<_, _>>(),
			body_template: body_template.map(str::to_string),
		}
	}

	fn ctx() -> ToolContext {
		ToolContext::default()
	}

	fn http_response(status: &str, body: &str, extra_headers: &str) -> String {
		format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra_headers}\r\n{body}", body.len())
	}

	fn ok_response() -> String {
		http_response("200 OK", "{\"ok\":true}", "Content-Type: application/json\r\n")
	}

	fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
		haystack.windows(needle.len()).position(|window| window == needle)
	}

	async fn read_request(socket: &mut TcpStream) -> String {
		let mut buf = Vec::new();
		let mut chunk = [0u8; 4096];
		loop {
			let n = socket.read(&mut chunk).await.unwrap();
			if n == 0 {
				break;
			}
			buf.extend_from_slice(&chunk[..n]);
			if let Some(head_end) = find_subsequence(&buf, b"\r\n\r\n") {
				let head = String::from_utf8_lossy(&buf[..head_end]).to_lowercase();
				let content_length = head
					.lines()
					.find_map(|line| line.strip_prefix("content-length:").and_then(|value| value.trim().parse::<usize>().ok()))
					.unwrap_or(0);
				if buf.len() >= head_end + 4 + content_length {
					break;
				}
			}
		}
		String::from_utf8_lossy(&buf).into_owned()
	}

	fn request_body(raw_request: &str) -> &str {
		raw_request.split("\r\n\r\n").nth(1).unwrap_or("")
	}

	struct TestServer {
		base_url: String,
		hits: Arc<AtomicUsize>,
		request: tokio::task::JoinHandle<String>,
	}

	async fn spawn_server(response: String) -> TestServer {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let addr = listener.local_addr().unwrap();
		let hits = Arc::new(AtomicUsize::new(0));
		let task_hits = Arc::clone(&hits);
		let request = tokio::spawn(async move {
			let (mut socket, _) = listener.accept().await.unwrap();
			task_hits.fetch_add(1, Ordering::SeqCst);
			let raw = read_request(&mut socket).await;
			socket.write_all(response.as_bytes()).await.unwrap();
			raw
		});
		TestServer {
			base_url: format!("http://{addr}"),
			hits,
			request,
		}
	}

	#[tokio::test]
	async fn json_body_placeholder_is_substituted_as_typed_escaped_value() {
		let server = spawn_server(ok_response()).await;
		let config = http_config(&format!("{}/hook", server.base_url), Some(r#"{"q": "{{input.q}}"}"#), &[]);
		let executor = HttpExecutor::new("t".to_string(), config).unwrap();

		let result = executor.execute(json!({ "q": "x\", \"evil\": true" }), &ctx()).await.unwrap();
		assert_eq!(result, json!({ "ok": true }));

		let raw = server.request.await.unwrap();
		let parsed: Value = serde_json::from_str(request_body(&raw)).unwrap();
		assert_eq!(parsed["q"], "x\", \"evil\": true");
		assert!(parsed.get("evil").is_none());
	}

	#[tokio::test]
	async fn json_body_keeps_non_string_types_and_escapes_embedded_placeholders() {
		let server = spawn_server(ok_response()).await;
		let config = http_config(
			&format!("{}/hook", server.base_url),
			Some(r#"{"n": "{{input.n}}", "s": "pre {{input.s}} post"}"#),
			&[],
		);
		let executor = HttpExecutor::new("t".to_string(), config).unwrap();

		executor.execute(json!({ "n": 42, "s": "va\"l" }), &ctx()).await.unwrap();

		let raw = server.request.await.unwrap();
		let parsed: Value = serde_json::from_str(request_body(&raw)).unwrap();
		assert_eq!(parsed["n"], 42);
		assert_eq!(parsed["s"], "pre va\"l post");
	}

	#[tokio::test]
	async fn non_json_body_template_uses_raw_substitution() {
		let server = spawn_server(ok_response()).await;
		let config = http_config(&format!("{}/hook", server.base_url), Some("q={{input.q}}"), &[]);
		let executor = HttpExecutor::new("t".to_string(), config).unwrap();

		executor.execute(json!({ "q": "x\", \"evil\": true" }), &ctx()).await.unwrap();

		let raw = server.request.await.unwrap();
		assert_eq!(request_body(&raw), "q=x\", \"evil\": true");
	}

	#[tokio::test]
	async fn url_host_switch_via_input_placeholder_is_rejected() {
		for malicious in ["//evil.com", "@evil.com", "https://evil.com/hook", "file:///etc/passwd"] {
			let executor = HttpExecutor::new("t".to_string(), http_config("{{input.x}}", None, &[])).unwrap();
			let result = executor.execute(json!({ "x": malicious }), &ctx()).await;
			assert!(matches!(result, Err(ToolError::InvalidInput(_))), "input {malicious:?} must be rejected");
		}

		let executor = HttpExecutor::new("t".to_string(), http_config("https://{{input.x}}/hook", None, &[])).unwrap();
		let result = executor.execute(json!({ "x": "evil.com" }), &ctx()).await;
		assert!(matches!(result, Err(ToolError::InvalidInput(_))));

		let executor = HttpExecutor::new("t".to_string(), http_config("https://api.example.com{{input.x}}", None, &[])).unwrap();
		let result = executor.execute(json!({ "x": "@evil.com" }), &ctx()).await;
		assert!(matches!(result, Err(ToolError::InvalidInput(_))));
	}

	#[tokio::test]
	async fn same_host_path_and_query_substitution_is_allowed() {
		let server = spawn_server(ok_response()).await;
		let url = format!("{}/{{{{input.path}}}}?q={{{{input.q}}}}", server.base_url);
		let executor = HttpExecutor::new("t".to_string(), http_config(&url, None, &[])).unwrap();

		let result = executor.execute(json!({ "path": "hook", "q": "a b" }), &ctx()).await.unwrap();
		assert_eq!(result, json!({ "ok": true }));

		let raw = server.request.await.unwrap();
		assert!(raw.contains("POST /hook?q=a%20b "), "unexpected request line in: {raw}");
	}

	#[tokio::test]
	async fn url_input_cannot_leave_its_path_segment_or_add_a_query() {
		let server = spawn_server(ok_response()).await;
		let url = format!("{}/items/{{{{input.id}}}}", server.base_url);
		let executor = HttpExecutor::new("t".to_string(), http_config(&url, None, &[])).unwrap();

		executor.execute(json!({ "id": "../../admin/users?all=1#" }), &ctx()).await.unwrap();
		let raw = server.request.await.unwrap();
		assert!(raw.contains("POST /items/..%2F..%2Fadmin%2Fusers%3Fall%3D1%23 "), "unexpected request line in: {raw}");

		for dots in ["..", ".", "%2e%2e"] {
			let server = spawn_server(ok_response()).await;
			let url = format!("{}/items/{{{{input.id}}}}/details", server.base_url);
			let executor = HttpExecutor::new("t".to_string(), http_config(&url, None, &[])).unwrap();
			let result = executor.execute(json!({ "id": dots }), &ctx()).await;
			if dots.starts_with('%') {
				result.unwrap();
				let raw = server.request.await.unwrap();
				assert!(raw.contains("POST /items/%252e%252e/details "), "unexpected request line in: {raw}");
			} else {
				assert!(matches!(result, Err(ToolError::InvalidInput(_))), "input {dots:?} must be rejected");
				assert_eq!(server.hits.load(Ordering::SeqCst), 0, "request must not be sent");
			}
		}
	}

	#[test]
	fn function_entrypoints_stay_on_the_tool_host() {
		use crate::utils::tools::http::function_url;

		assert_eq!(function_url("https://api.example.com/v1/", None).unwrap(), "https://api.example.com/v1/");
		assert_eq!(function_url("https://api.example.com/v1/", Some("/search")).unwrap(), "https://api.example.com/v1/search");
		assert_eq!(function_url("https://api.example.com/v1", Some("https://api.example.com/v2/search")).unwrap(), "https://api.example.com/v2/search");
		for foreign in ["https://evil.example/collect", "http://api.example.com/v1", "https://api.example.com:8443/v1", "https://api.example.com.evil.example/"] {
			assert!(matches!(function_url("https://api.example.com/v1", Some(foreign)), Err(ToolError::InvalidInput(_))), "{foreign} must be refused");
		}
	}

	#[test]
	fn exa_region_picks_the_endpoint() {
		use crate::utils::tools::builtin::websearch::exa_base_url;

		assert_eq!(exa_base_url(Some("eu")), "https://api.eu.exa.ai");
		for region in [None, Some("global"), Some("EU"), Some("us")] {
			assert_eq!(exa_base_url(region), "https://api.exa.ai", "{region:?} must use the global endpoint");
		}
	}

	#[test]
	fn a_config_that_moves_host_must_not_keep_stored_secrets() {
		use crate::utils::encryption::config_redirects_secrets;

		let existing = json!({"url": "https://api.example.com/mcp", "headers": {"Authorization": "Bearer stored"}});
		let moved_masked = json!({"url": "https://evil.example/mcp", "headers": {"Authorization": "***"}});
		let moved_retyped = json!({"url": "https://evil.example/mcp", "headers": {"Authorization": "Bearer new"}});
		let moved_without_headers = json!({"url": "https://evil.example/mcp"});
		let same_host_masked = json!({"url": "https://api.example.com/other", "headers": {"Authorization": "***"}});

		assert!(config_redirects_secrets(&existing, &moved_masked, &["headers", "env"]));
		assert!(!config_redirects_secrets(&existing, &moved_retyped, &["headers", "env"]));
		assert!(!config_redirects_secrets(&existing, &moved_without_headers, &["headers", "env"]));
		assert!(!config_redirects_secrets(&existing, &same_host_masked, &["headers", "env"]));
	}

	#[tokio::test]
	async fn oversized_response_bodies_are_refused() {
		use crate::utils::tools::http::MAX_TOOL_RESPONSE_BYTES;

		let body = "x".repeat(MAX_TOOL_RESPONSE_BYTES + 1);
		let server = spawn_server(http_response("200 OK", &body, "Content-Type: text/plain\r\n")).await;
		let executor = HttpExecutor::new("t".to_string(), http_config(&format!("{}/hook", server.base_url), None, &[])).unwrap();
		let result = executor.execute(json!({}), &ctx()).await;
		assert!(matches!(result, Err(ToolError::HttpError(_))), "unexpected result: {:?}", result.map(|_| ()));
	}

	#[tokio::test]
	async fn connection_errors_do_not_reveal_the_tool_url() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let address = listener.local_addr().unwrap();
		drop(listener);
		let config = http_config(&format!("http://{address}/hook?api_key={{{{settings.key}}}}"), None, &[]);
		let executor = HttpExecutor::new("t".to_string(), config).unwrap();
		let ctx = ToolContext {
			settings: json!({ "key": "s3cret-value" }),
			..ToolContext::default()
		};
		let error = executor.execute(json!({}), &ctx).await.unwrap_err().to_string();
		assert!(!error.contains("s3cret-value") && !error.contains("/hook"), "error reveals the URL: {error}");
	}

	#[test]
	fn image_fetches_use_the_shared_address_filter() {
		use crate::utils::outbound::is_allowed_ip;

		for blocked in ["100.64.0.1", "169.254.169.254", "10.0.0.1", "127.0.0.1", "64:ff9b::a00:1", "2002:a00:1::1"] {
			assert!(!is_allowed_ip(blocked.parse().unwrap(), false), "{blocked} must be refused");
		}
		assert!(is_allowed_ip("93.184.216.34".parse().unwrap(), false));
	}

	#[tokio::test]
	async fn redirect_responses_are_not_followed() {
		let target = spawn_server(ok_response()).await;
		let redirect_response = http_response("302 Found", "", &format!("Location: {}/hook\r\n", target.base_url));
		let redirector = spawn_server(redirect_response).await;

		let executor = HttpExecutor::new("t".to_string(), http_config(&format!("{}/hook", redirector.base_url), None, &[])).unwrap();
		let result = executor.execute(json!({}), &ctx()).await;

		let err = result.unwrap_err();
		assert!(err.to_string().contains("302"), "unexpected error: {err}");
		assert!(!err.to_string().contains("ok"));
		assert_eq!(target.hits.load(Ordering::SeqCst), 0, "redirect target must not be contacted");
		redirector.request.await.unwrap();
	}

	#[tokio::test]
	async fn upstream_error_body_is_not_returned_to_caller() {
		let server = spawn_server(http_response("500 Internal Server Error", "SECRET-UPSTREAM-BODY", "")).await;
		let executor = HttpExecutor::new("t".to_string(), http_config(&format!("{}/hook", server.base_url), None, &[])).unwrap();

		let err = executor.execute(json!({}), &ctx()).await.unwrap_err();
		let msg = err.to_string();
		assert!(msg.contains("500"), "unexpected error: {msg}");
		assert!(!msg.contains("SECRET-UPSTREAM-BODY"), "upstream body leaked into error: {msg}");
		server.request.await.unwrap();
	}

	#[tokio::test]
	async fn substituted_header_value_with_control_characters_is_rejected() {
		let server = spawn_server(ok_response()).await;
		let config = http_config(&format!("{}/hook", server.base_url), None, &[("X-API-Key", "{{input.key}}")]);
		let executor = HttpExecutor::new("t".to_string(), config).unwrap();

		let result = executor.execute(json!({ "key": "abc\r\nX-Injected: 1" }), &ctx()).await;
		assert!(matches!(result, Err(ToolError::InvalidInput(_))));
		assert_eq!(server.hits.load(Ordering::SeqCst), 0, "request must not be sent");
	}

	#[tokio::test]
	async fn header_settings_substitution_still_works() {
		let server = spawn_server(ok_response()).await;
		let config = http_config(&format!("{}/hook", server.base_url), None, &[("X-Key", "{{settings.key}}")]);
		let executor = HttpExecutor::new("t".to_string(), config).unwrap();
		let ctx = ToolContext {
			settings: json!({ "key": "s3cret" }),
			..ToolContext::default()
		};

		executor.execute(json!({}), &ctx).await.unwrap();

		let raw = server.request.await.unwrap();
		assert!(raw.to_lowercase().contains("x-key: s3cret"), "missing header in: {raw}");
	}
}
