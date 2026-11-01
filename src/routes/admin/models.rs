use crate::types::consts::{ADMIN_PROVIDERS_EDIT, ADMIN_PROVIDERS_VIEW};
use crate::types::{AppError, AuthUser, JobState, PageQuery};
use crate::types::models::{AdminModelPatchBody, Model, ModelListParams, ModelPricing, ModelPricingOverrideRequest};
use crate::types::models_configs::{ModelConfig, ModelConfigPatchField};
use crate::utils::images::{image_url, is_data_uri, store_from_data_uri};
use crate::utils::response::{ErrorCode, ResponseBody, ResponseBuilder};
use axum::extract::Query;
use axum::{
	Json,
	extract::{Path, State},
	response::Response,
};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

/// Returns true when the string contains an HTML event-handler attribute like `onload=`.
fn has_event_handler(value: &str) -> bool {
	let lower = value.to_ascii_lowercase();
	let bytes = lower.as_bytes();
	for i in 0..bytes.len().saturating_sub(2) {
		if bytes[i] == b'o' && bytes[i + 1] == b'n' && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric()) {
			let mut j = i + 2;
			while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
				j += 1;
			}
			if j > i + 2 && j < bytes.len() && bytes[j] == b'=' {
				return true;
			}
		}
	}
	false
}

/// Validates a raw SVG icon string before it is stored and later rendered via `v-html`.
///
/// Data URIs are exempt (they are sniffed as raster images and replaced by an internal
/// image URL). Raw strings must be benign SVG: start with `<svg` and contain no
/// `<script`, `javascript:`, or event-handler attributes.
fn validate_icon_svg(value: &str) -> bool {
	let trimmed = value.trim();
	if trimmed.is_empty() {
		return true;
	}
	let lower = trimmed.to_ascii_lowercase();
	lower.starts_with("<svg") && !lower.contains("<script") && !lower.contains("javascript:") && !has_event_handler(trimmed)
}

/// Merges `reasoning_effort` and `reasoning_budget_tokens` into the `extra_settings` JSON blob.
///
/// Returns `None` if neither reasoning field was included in the request (meaning
/// `extra_settings` itself should not be touched).
fn build_extra_settings(base: Option<&Value>, reasoning_effort: Option<Option<&str>>, reasoning_budget_tokens: Option<Option<u32>>) -> Option<Value> {
	if reasoning_effort.is_none() && reasoning_budget_tokens.is_none() {
		return None;
	}

	let mut map = if let Some(Value::Object(obj)) = base {
		obj.clone()
	} else {
		serde_json::Map::new()
	};

	match reasoning_effort {
		Some(Some(effort)) => {
			map.insert("reasoning_effort".to_string(), Value::String(effort.to_string()));
		}
		Some(None) => {
			map.remove("reasoning_effort");
		}
		None => {}
	}

	match reasoning_budget_tokens {
		Some(Some(budget)) => {
			map.insert("reasoning_budget_tokens".to_string(), Value::Number(budget.into()));
		}
		Some(None) => {
			map.remove("reasoning_budget_tokens");
		}
		None => {}
	}

	Some(Value::Object(map))
}

/// GET /api/v1/admin/models
pub async fn list_models(State(state): State<Arc<JobState>>, auth: AuthUser, Query(page): Query<PageQuery>, Query(params): Query<ModelListParams>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let models = Model::list_for_admin(&state.db, &page, params.query).await?;

	Ok(ResponseBuilder::new(ResponseBody::Json(models)).build())
}

pub async fn list_image_models(State(state): State<Arc<JobState>>, auth: AuthUser, Query(page): Query<PageQuery>, Query(params): Query<ModelListParams>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let models = Model::list_image_models_for_admin(&state.db, &page, params.query, params.provider_id).await?;

	Ok(ResponseBuilder::new(ResponseBody::Json(models)).build())
}

/// GET /api/v1/admin/models/:id
pub async fn get_model(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let row = Model::find_for_admin(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;

	Ok(ResponseBuilder::new(ResponseBody::Json(row)).build())
}

pub async fn list_image_model_providers(State(state): State<Arc<JobState>>, auth: AuthUser) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let providers = Model::list_image_providers_for_admin(&state.db).await?;

	Ok(ResponseBuilder::new(ResponseBody::Json(providers)).build())
}

pub async fn get_model_pricing(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_VIEW).await?;

	let pricing = ModelPricing::effective(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;

	Ok(ResponseBuilder::new(ResponseBody::Json(pricing)).build())
}

pub async fn put_model_pricing(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<ModelPricingOverrideRequest>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	Model::find_by_id(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;
	let pricing_json = serde_json::to_value(&req.pricing).map_err(|error| {
		tracing::error!("Failed to serialize pricing override: {error}");
		ErrorCode::InternalError
	})?;
	let pricing = ModelPricing::upsert_override(&state.db, &id, &pricing_json).await?;

	Ok(ResponseBuilder::new(ResponseBody::Json(pricing)).build())
}

pub async fn delete_model_pricing(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	Model::find_by_id(&state.db, &id).await?.ok_or(ErrorCode::NotFound)?;
	ModelPricing::delete_override(&state.db, &id).await?;

	Ok(ResponseBuilder::<()>::new(ResponseBody::Empty).build())
}

/// PATCH /api/v1/admin/models/:id
#[allow(clippy::cognitive_complexity)]
pub async fn patch_model(State(state): State<Arc<JobState>>, auth: AuthUser, Path(id): Path<Uuid>, Json(req): Json<AdminModelPatchBody>) -> Result<Response, AppError> {
	auth.require(&state.db, ADMIN_PROVIDERS_EDIT).await?;

	let icon: Option<Option<String>> = match req.icon {
		Some(Some(ref s)) if is_data_uri(s) => {
			let stored = store_from_data_uri(&state.db, s, None, Some("model_icon")).await.map_err(|error| {
				tracing::error!("Failed to store model icon: {error}");
				ErrorCode::InternalError
			})?;
			Some(Some(image_url(stored.id)))
		}
		Some(ref inner) => match inner.as_deref() {
			Some(s) if !validate_icon_svg(s) => return Err(ErrorCode::ValidationFailed.into()),
			other => Some(other.map(str::to_owned)),
		},
		None => None,
	};

	let mut tx = state.db.begin().await?;

	if req.display_name.is_some() || req.is_enabled.is_some() {
		Model::patch(&mut tx, &id, req.display_name.as_deref(), req.is_enabled).await?.ok_or(ErrorCode::NotFound)?;
	}

	let (m_name, m_id) = Model::find_name_and_model_id(&mut *tx, &id).await?.ok_or(ErrorCode::NotFound)?;
	ModelConfig::ensure_system_config(&mut tx, &id, &m_id, &m_name).await?;

	let extra_settings = build_extra_settings(
		req.extra_settings.as_ref().and_then(|v| v.as_ref()),
		req.reasoning_effort.as_ref().map(|v| v.as_deref()),
		req.reasoning_budget_tokens.as_ref().map(|v| *v),
	);

	let mut config_fields: Vec<ModelConfigPatchField<'_>> = Vec::new();

	if let Some(ref v) = req.description {
		config_fields.push(ModelConfigPatchField::Description(v.as_deref()));
	}
	if let Some(ref v) = icon {
		config_fields.push(ModelConfigPatchField::Icon(v.as_deref()));
	}
	if let Some(ref v) = req.system_prompt {
		config_fields.push(ModelConfigPatchField::SystemPrompt(v.as_deref()));
	}
	if let Some(Some(ref v)) = req.sampling {
		config_fields.push(ModelConfigPatchField::SamplingMerge(v));
	}
	if let Some(ref v) = req.input_modalities {
		config_fields.push(ModelConfigPatchField::InputModalities(v.as_ref()));
	}
	if let Some(ref v) = req.output_modalities {
		config_fields.push(ModelConfigPatchField::OutputModalities(v.as_ref()));
	}
	if let Some(v) = req.context_length {
		config_fields.push(ModelConfigPatchField::ContextLength(v));
	}
	if let Some(v) = req.max_output_tokens {
		config_fields.push(ModelConfigPatchField::MaxOutputTokens(v));
	}
	if let Some(ref v) = req.enabled_tools {
		config_fields.push(ModelConfigPatchField::EnabledTools(v.as_ref()));
	}
	if let Some(v) = req.is_public {
		config_fields.push(ModelConfigPatchField::IsPublic(v));
	}
	if let Some(v) = req.is_featured {
		config_fields.push(ModelConfigPatchField::IsFeatured(v));
	}
	if let Some(v) = req.is_default {
		config_fields.push(ModelConfigPatchField::IsDefault(v));
	}
	if let Some(v) = req.is_favorite {
		config_fields.push(ModelConfigPatchField::IsFavorite(v));
	}
	if let Some(ref v) = req.category {
		config_fields.push(ModelConfigPatchField::Category(v.as_deref()));
	}
	if let Some(ref v) = req.tags {
		config_fields.push(ModelConfigPatchField::Tags(v.as_ref()));
	}
	if let Some(ref v) = extra_settings {
		config_fields.push(ModelConfigPatchField::ExtraSettings(Some(v)));
	}

	if !config_fields.is_empty() {
		ModelConfig::patch_system_config(&mut tx, &id, &config_fields).await?;
	}

	tx.commit().await?;

	Ok(ResponseBuilder::<()>::new(ResponseBody::Empty).build())
}
