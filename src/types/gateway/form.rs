use axum::body::{Body, Bytes};
use axum::extract::{FromRequest, Multipart, Request, multipart::MultipartRejection};
use axum::http::{Extensions, HeaderValue, StatusCode, header::CONTENT_TYPE};
use serde_json::{Map, Value};
use uuid::Uuid;

// Internal Types

struct FormPart {
	name: String,
	file_name: Option<String>,
	content_type: Option<String>,
	bytes: Bytes,
}

/// A multipart gateway request read into memory, so its `model` can be resolved and rewritten
/// before Omniference's skin parses the form.
pub struct GatewayForm {
	parts: Vec<FormPart>,
	extensions: Extensions,
}

/// A form that could not be read, with the status and message to answer it with.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct GatewayFormError {
	pub status: StatusCode,
	pub message: String,
}

impl GatewayForm {
	/// Reads every part of a multipart request within the route's body limit.
	///
	/// # Errors
	/// Returns the multipart rejection or read failure.
	pub async fn read(request: Request) -> Result<Self, GatewayFormError> {
		let extensions = request.extensions().clone();
		let mut multipart = Multipart::from_request(request, &()).await.map_err(|rejection| GatewayFormError {
			status: rejection.status(),
			message: rejection.body_text(),
		})?;
		let mut parts = Vec::new();
		loop {
			let field = multipart.next_field().await.map_err(|error| GatewayFormError {
				status: error.status(),
				message: error.body_text(),
			})?;
			let Some(field) = field else { break };
			let name = field.name().unwrap_or_default().to_owned();
			let file_name = field.file_name().map(str::to_owned);
			let content_type = field.content_type().map(str::to_owned);
			let bytes = field.bytes().await.map_err(|error| GatewayFormError {
				status: error.status(),
				message: error.body_text(),
			})?;
			parts.push(FormPart {
				name,
				file_name,
				content_type,
				bytes,
			});
		}
		Ok(Self { parts, extensions })
	}

	#[must_use]
	/// The first value of a text field.
	pub fn text(&self, name: &str) -> Option<String> {
		self.parts.iter().find(|part| part.name == name).map(|part| String::from_utf8_lossy(&part.bytes).into_owned())
	}

	/// Replaces every value of a text field with one value.
	pub fn set_text(&mut self, name: &str, value: &str) {
		self.parts.retain(|part| part.name != name);
		self.parts.push(FormPart {
			name: name.to_owned(),
			file_name: None,
			content_type: None,
			bytes: Bytes::copy_from_slice(value.as_bytes()),
		});
	}

	#[must_use]
	/// Every part not named in `files`, as text, with a repeated name as an array.
	pub fn fields(&self, files: &[&str]) -> Map<String, Value> {
		let mut fields = Map::new();
		for part in self.parts.iter().filter(|part| !files.contains(&part.name.as_str())) {
			let text = Value::String(String::from_utf8_lossy(&part.bytes).into_owned());
			match fields.get_mut(&part.name) {
				Some(Value::Array(items)) => items.push(text),
				Some(existing) => *existing = Value::Array(vec![existing.take(), text]),
				None => {
					fields.insert(part.name.clone(), text);
				}
			}
		}
		fields
	}

	#[must_use]
	/// How many parts are named in `files` and their total size in bytes.
	pub fn files(&self, files: &[&str]) -> (u64, u64) {
		self.parts
			.iter()
			.filter(|part| files.contains(&part.name.as_str()))
			.fold((0, 0), |(count, bytes), part| (count + 1, bytes + part.bytes.len() as u64))
	}

	/// Encodes the form again as the multipart request the skin reads, under the original request's
	/// extensions so the route's body limit still applies.
	///
	/// # Errors
	/// Returns the rejection the multipart extractor reports.
	pub async fn into_multipart(self) -> Result<Multipart, MultipartRejection> {
		let boundary = format!("oxide-{}", Uuid::new_v4().simple());
		let size = self.parts.iter().map(|part| part.bytes.len() + 256).sum::<usize>() + 64;
		let mut body = Vec::with_capacity(size);
		for part in &self.parts {
			body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"", quoted(&part.name)).as_bytes());
			if let Some(file_name) = &part.file_name {
				body.extend_from_slice(format!("; filename=\"{}\"", quoted(file_name)).as_bytes());
			}
			body.extend_from_slice(b"\r\n");
			if let Some(content_type) = &part.content_type {
				body.extend_from_slice(format!("Content-Type: {}\r\n", content_type.replace(['\r', '\n'], " ")).as_bytes());
			}
			body.extend_from_slice(b"\r\n");
			body.extend_from_slice(&part.bytes);
			body.extend_from_slice(b"\r\n");
		}
		body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
		let mut request = Request::new(Body::from(body));
		*request.extensions_mut() = self.extensions;
		if let Ok(value) = HeaderValue::from_str(&format!("multipart/form-data; boundary={boundary}")) {
			request.headers_mut().insert(CONTENT_TYPE, value);
		}
		Multipart::from_request(request, &()).await
	}
}

fn quoted(value: &str) -> String {
	value.replace('"', "%22").replace(['\r', '\n'], " ")
}
