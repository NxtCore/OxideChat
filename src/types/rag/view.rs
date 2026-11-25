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
	/// The payload field that lists who may see each record (labels such as `HR Members`); `None`: everyone who may use the knowledge base sees every record.
	#[serde(default)]
	pub access: Option<String>,
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
		validate_collection(&self.collection)?;
		let optional = [&self.vector_name, &self.fields.title, &self.fields.url, &self.fields.section, &self.access];
		if !valid_path(&self.fields.text) || optional.into_iter().flatten().any(|path| !valid_path(path)) {
			return Err(RagError::InvalidField);
		}
		validate_filter(&self.filter)
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
			page: None,
			score,
		})
	}
}

/// Checks a collection name as views store it.
///
/// # Errors
///
/// Returns `InvalidCollection` for an empty or too long name.
pub fn validate_collection(collection: &str) -> Result<(), RagError> {
	if collection.trim().is_empty() || collection.len() > 255 { Err(RagError::InvalidCollection) } else { Ok(()) }
}

/// Checks a fixed filter: at most [`MAX_FILTERS`] conditions on valid field paths with scalar values.
///
/// # Errors
///
/// Returns `InvalidFilter` otherwise.
pub fn validate_filter(filter: &[FieldFilter]) -> Result<(), RagError> {
	let scalar = |value: &Value| matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_));
	if filter.len() > MAX_FILTERS || filter.iter().any(|condition| !valid_path(&condition.field) || !scalar(&condition.value)) {
		Err(RagError::InvalidFilter)
	} else {
		Ok(())
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
	/// 1-based page number for managed PDFs.
	#[serde(default)]
	pub page: Option<i32>,
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

/// The column a field names and the path inside it: `site` is a column, `metadata.title` reaches into the `metadata` column.
fn pg_column<'a>(field: &'a str, columns: &'a [String]) -> Option<(&'a String, Option<&'a str>)> {
	if let Some(column) = columns.iter().find(|column| **column == field) {
		return Some((column, None));
	}
	let (column, path) = field.split_once('.')?;
	Some((columns.iter().find(|known| *known == column)?, Some(path)))
}

/// The labels of `field` as JSON, for discovery and for matching JSON columns; `$path` is the path inside the column as `text[]`.
#[must_use]
pub fn pg_access_value(field: &str, columns: &[String], path: usize) -> Option<(String, Vec<String>)> {
	let (column, inner) = pg_column(field, columns)?;
	let segments = inner.map(|inner| inner.split('.').map(str::to_string).collect()).unwrap_or_default();
	Some((format!("(to_jsonb(t.{}) #> ${path}::text[])", quote_ident(column)), segments))
}

/// The condition that keeps records whose `field` holds one of the labels in `$labels`, and the path parameter at `$path`.
///
/// A `text[]` column is compared with `&&`, so a GIN index on it applies, and a `varchar[]` one after a cast to `text[]`;
/// anything else is read as JSON, where a single string counts as one label.
/// A record without the field never matches.
#[must_use]
pub fn pg_access(field: &str, columns: &[(String, String, i32)], labels: usize, path: usize) -> Option<(String, Vec<String>)> {
	let names: Vec<String> = columns.iter().map(|(name, _, _)| name.clone()).collect();
	let array = columns.iter().find(|(name, kind, _)| name == field && matches!(kind.as_str(), "_text" | "_varchar"));
	if let Some((_, kind, _)) = array {
		let cast = if kind == "_varchar" { "::text[]" } else { "" };
		return Some((format!("t.{}{cast} && ${labels}::text[]", quote_ident(field)), Vec::new()));
	}
	let (value, segments) = pg_access_value(field, &names, path)?;
	Some((format!("(jsonb_typeof({value}) IN ('array', 'string') AND {value} ?| ${labels}::text[])"), segments))
}

/// The Qdrant condition that keeps records whose `field` holds one of `labels`.
#[must_use]
pub fn qdrant_access(field: &str, labels: &[String]) -> Value {
	serde_json::json!({ "key": field, "match": { "any": labels } })
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
		let (column, path) = pg_column(&condition.field, columns)?;
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

/// The Qdrant filter of a view plus `extra` conditions, or `None` without any.
#[must_use]
pub fn qdrant_filter(filter: &[FieldFilter], extra: Option<Value>) -> Option<Value> {
	let must: Vec<Value> = filter
		.iter()
		.map(|condition| serde_json::json!({ "key": condition.field, "match": { "value": condition.value } }))
		.chain(extra)
		.collect();
	(!must.is_empty()).then(|| serde_json::json!({ "must": must }))
}
