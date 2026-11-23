//! How a knowledge base reads a collection someone else fills: which vector, which model built it and which payload fields hold the text.

use super::{EmbeddingSettings, RagError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Most fixed filter conditions a view can have.
pub const MAX_FILTERS: usize = 10;

/// Longest text a sample record shows per field.
pub const SAMPLE_TEXT_CHARS: usize = 500;

/// How a collection measures how close two vectors are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum Distance {
	#[default]
	Cosine,
	Dot,
	Euclid,
	Manhattan,
}

impl Distance {
	/// The pgvector operator; every one sorts nearest first.
	#[must_use]
	pub const fn pg_operator(self) -> &'static str {
		match self {
			Self::Cosine => "<=>",
			Self::Dot => "<#>",
			Self::Euclid => "<->",
			Self::Manhattan => "<+>",
		}
	}

	/// Turns a pgvector distance into a score where higher is closer.
	#[must_use]
	pub fn pg_score(self, distance: f64) -> f64 {
		match self {
			Self::Cosine => 1.0 - distance,
			Self::Dot | Self::Euclid | Self::Manhattan => -distance,
		}
	}

	/// Turns a Qdrant score into one where higher is closer; Qdrant reports Euclid and Manhattan as distances.
	#[must_use]
	pub fn qdrant_score(self, score: f64) -> f64 {
		match self {
			Self::Cosine | Self::Dot => score,
			Self::Euclid | Self::Manhattan => -score,
		}
	}

	/// Parses Qdrant's name (`Cosine`, `Dot`, `Euclid`, `Manhattan`).
	#[must_use]
	pub fn from_qdrant(name: &str) -> Option<Self> {
		match name {
			"Cosine" => Some(Self::Cosine),
			"Dot" => Some(Self::Dot),
			"Euclid" => Some(Self::Euclid),
			"Manhattan" => Some(Self::Manhattan),
			_ => None,
		}
	}
}

/// Payload fields that hold a hit's text and, optionally, its title, link and section; dotted paths reach into objects.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FieldMapping {
	pub text: String,
	#[serde(default)]
	pub title: Option<String>,
	#[serde(default)]
	pub url: Option<String>,
	#[serde(default)]
	pub section: Option<String>,
}

/// One fixed condition every search of the view adds: `field` equals `value`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct FieldFilter {
	pub field: String,
	/// A string, number or boolean.
	pub value: Value,
}

/// A knowledge base on a read-only connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ExternalView {
	pub connection_id: Uuid,
	/// Qdrant collection or pgvector `schema.table`.
	pub collection: String,
	/// Qdrant named vector or pgvector column; `None` is Qdrant's unnamed vector.
	#[serde(default)]
	pub vector_name: Option<String>,
	/// Qdrant reports its own; pgvector searches with this one.
	#[serde(default)]
	pub distance: Distance,
	/// The model that built the collection; its vectors must have the collection's size.
	pub embedding: EmbeddingSettings,
	pub fields: FieldMapping,
	#[serde(default)]
	pub filter: Vec<FieldFilter>,
}

fn valid_path(path: &str) -> bool {
	!path.trim().is_empty() && path.len() <= 200
}

impl ExternalView {
	/// Checks the fields that need no store.
	///
	/// # Errors
	///
	/// Returns `InvalidCollection`, `InvalidField` or `InvalidFilter`.
	pub fn validate(&self) -> Result<(), RagError> {
		if self.collection.trim().is_empty() || self.collection.len() > 255 {
			return Err(RagError::InvalidCollection);
		}
		let optional = [&self.vector_name, &self.fields.title, &self.fields.url, &self.fields.section];
		if !valid_path(&self.fields.text) || optional.into_iter().flatten().any(|path| !valid_path(path)) {
			return Err(RagError::InvalidField);
		}
		let scalar = |value: &Value| matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_));
		if self.filter.len() > MAX_FILTERS || self.filter.iter().any(|filter| !valid_path(&filter.field) || !scalar(&filter.value)) {
			return Err(RagError::InvalidFilter);
		}
		Ok(())
	}

	/// The text, title, link and section of one record, or `None` when it has no text.
	#[must_use]
	pub fn hit(&self, payload: &Value, score: f64) -> Option<KnowledgeHit> {
		let read = |path: &Option<String>| path.as_deref().and_then(|path| field_text(payload, path));
		Some(KnowledgeHit {
			text: field_text(payload, &self.fields.text).filter(|text| !text.trim().is_empty())?,
			title: read(&self.fields.title),
			url: read(&self.fields.url).filter(|url| url.starts_with("https://") || url.starts_with("http://")),
			section: read(&self.fields.section),
			score,
		})
	}
}

/// One search result as the knowledge base maps it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct KnowledgeHit {
	pub text: String,
	pub title: Option<String>,
	pub url: Option<String>,
	pub section: Option<String>,
	/// Higher is closer; only comparable within one knowledge base.
	pub score: f64,
}

/// The value at a dotted `path` as text; strings, numbers and booleans only.
#[must_use]
pub fn field_text(payload: &Value, path: &str) -> Option<String> {
	match path.split('.').try_fold(payload, |value, key| value.get(key))? {
		Value::String(text) => Some(text.clone()),
		value @ (Value::Number(_) | Value::Bool(_)) => Some(value.to_string()),
		_ => None,
	}
}

/// Every scalar field path in `records`, up to one level of nesting, sorted.
#[must_use]
pub fn field_paths(records: &[Value]) -> Vec<String> {
	let mut paths: Vec<String> = records
		.iter()
		.filter_map(Value::as_object)
		.flat_map(|record| {
			record.iter().flat_map(|(key, value)| match value {
				Value::Object(inner) => inner
					.iter()
					.filter(|(_, value)| !value.is_object())
					.map(|(child, _)| format!("{key}.{child}"))
					.collect(),
				_ => vec![key.clone()],
			})
		})
		.collect();
	paths.sort();
	paths.dedup();
	paths
}

/// Cuts every string in a sample record to [`SAMPLE_TEXT_CHARS`] characters.
#[must_use]
pub fn shorten(value: Value) -> Value {
	match value {
		Value::String(text) if text.chars().count() > SAMPLE_TEXT_CHARS => Value::String(text.chars().take(SAMPLE_TEXT_CHARS).chain(std::iter::once('…')).collect()),
		Value::Array(items) => Value::Array(items.into_iter().map(shorten).collect()),
		Value::Object(map) => Value::Object(map.into_iter().map(|(key, value)| (key, shorten(value))).collect()),
		value => value,
	}
}

/// A quoted Postgres identifier.
#[must_use]
pub fn quote_ident(name: &str) -> String {
	format!("\"{}\"", name.replace('"', "\"\""))
}

/// The `WHERE` clause of a pgvector view and its JSON parameters, numbered from `first`; `None` when a field does not start with one of `columns`.
///
/// Values compare as JSON, so `5` matches `5.0`; a dotted path reaches into a JSON column like the field mapping does.
#[must_use]
pub fn pg_filter(filter: &[FieldFilter], columns: &[String], first: usize) -> Option<(String, Vec<String>)> {
	let mut conditions = Vec::with_capacity(filter.len());
	let mut params = Vec::with_capacity(filter.len());
	for (offset, condition) in filter.iter().enumerate() {
		let (column, path) = match columns.iter().find(|column| **column == condition.field) {
			Some(column) => (column, None),
			None => {
				let (column, path) = condition.field.split_once('.')?;
				(columns.iter().find(|known| *known == column)?, Some(path))
			}
		};
		let parameter = first + offset;
		match path {
			None => {
				conditions.push(format!("to_jsonb(t.{}) = ${parameter}::jsonb", quote_ident(column)));
				params.push(condition.value.to_string());
			}
			Some(path) => {
				conditions.push(format!("to_jsonb(t.{}) @> ${parameter}::jsonb", quote_ident(column)));
				let nested = path.rsplit('.').fold(condition.value.clone(), |value, key| serde_json::json!({ key: value }));
				params.push(nested.to_string());
			}
		}
	}
	let clause = if conditions.is_empty() {
		String::new()
	} else {
		format!(" WHERE {}", conditions.join(" AND "))
	};
	Some((clause, params))
}

/// The Qdrant filter of a view, or `None` without conditions.
#[must_use]
pub fn qdrant_filter(filter: &[FieldFilter]) -> Option<Value> {
	(!filter.is_empty()).then(|| {
		let must: Vec<Value> = filter
			.iter()
			.map(|condition| serde_json::json!({ "key": condition.field, "match": { "value": condition.value } }))
			.collect();
		serde_json::json!({ "must": must })
	})
}
