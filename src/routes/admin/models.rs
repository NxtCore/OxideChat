use crate::routes::public::auth::get_current_user;
use crate::types::JobState;
use crate::types::models::{AdminModelPatchBody, Model, ModelListParams, ModelPricing, ModelPricingOverrideRequest};
use crate::types::models_configs::{ModelConfig, ModelConfigPatchField};
use crate::utils::images::{image_url, is_data_uri, store_from_data_uri};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::extract::Query;
use axum::{
	Json,
	extract::{Path, State},
	response::IntoResponse,
};
use serde_json::Value;
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

const ADMIN_MODELS_VIEW: &str = "admin.providers.view";
const ADMIN_MODELS_EDIT: &str = "admin.providers.edit";

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
pub async fn list_models(State(state): State<Arc<JobState>>, cookies: Cookies, Query(params): Query<ModelListParams>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_MODELS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let models = match Model::list_for_admin(&state.db, params.page.unwrap_or(1), params.size.unwrap_or(0), params.query).await {
		Ok(m) => m,
		Err(e) => {
			eprintln!("[ADMIN] Failed to list models: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	ResponseBuilder::new(ResponseBody::Json(models)).build()
}

pub async fn list_image_models(State(state): State<Arc<JobState>>, cookies: Cookies, Query(params): Query<ModelListParams>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};
	if !user.has_permission(&state.db, ADMIN_MODELS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	match Model::list_image_models_for_admin(
		&state.db,
		params.page.unwrap_or(1),
		params.size.unwrap_or(0),
		params.query,
		params.provider_id,
	)
	.await
	{
		Ok(models) => ResponseBuilder::new(ResponseBody::Json(models)).build(),
		Err(error) => {
			eprintln!("[ADMIN] Failed to list image models: {error}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// GET /api/v1/admin/models/:id
pub async fn get_model(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_MODELS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let row = match Model::find_for_admin(&state.db, &id).await {
		Ok(Some(r)) => r,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[ADMIN] Failed to get model: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	ResponseBuilder::new(ResponseBody::Json(row)).build()
}

pub async fn list_image_model_providers(State(state): State<Arc<JobState>>, cookies: Cookies) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};
	if !user.has_permission(&state.db, ADMIN_MODELS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	match Model::list_image_providers_for_admin(&state.db).await {
		Ok(providers) => ResponseBuilder::new(ResponseBody::Json(providers)).build(),
		Err(error) => {
			eprintln!("[ADMIN] Failed to list image-model providers: {error}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn get_model_pricing(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};
	if !user.has_permission(&state.db, ADMIN_MODELS_VIEW).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	match ModelPricing::effective(&state.db, &id).await {
		Ok(Some(pricing)) => ResponseBuilder::new(ResponseBody::Json(pricing)).build(),
		Ok(None) => ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[ADMIN] Failed to get model pricing: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn put_model_pricing(
	State(state): State<Arc<JobState>>,
	cookies: Cookies,
	Path(id): Path<Uuid>,
	Json(req): Json<ModelPricingOverrideRequest>,
) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};
	if !user.has_permission(&state.db, ADMIN_MODELS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	let model = match Model::find_by_id(&state.db, &id).await {
		Ok(Some(model)) => model,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[ADMIN] Failed to load model for pricing: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};
	let pricing_json = match serde_json::to_value(&req.pricing) {
		Ok(value) => value,
		Err(e) => {
			eprintln!("[ADMIN] Failed to serialize pricing override: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};
	match ModelPricing::upsert_override(&state.db, &id, &pricing_json).await {
		Ok(pricing) => ResponseBuilder::new(ResponseBody::Json(pricing)).build(),
		Err(e) => {
			eprintln!("[ADMIN] Failed to save model pricing: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

pub async fn delete_model_pricing(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};
	if !user.has_permission(&state.db, ADMIN_MODELS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}
	let model = match Model::find_by_id(&state.db, &id).await {
		Ok(Some(model)) => model,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[ADMIN] Failed to load model for pricing: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};
	match ModelPricing::delete_override(&state.db, &id).await {
		Ok(()) => ResponseBuilder::<()>::new(ResponseBody::Empty).build(),
		Err(e) => {
			eprintln!("[ADMIN] Failed to delete model pricing: {e}");
			ErrorBuilder::new(ErrorCode::InternalError).build()
		}
	}
}

/// PATCH /api/v1/admin/models/:id
pub async fn patch_model(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<AdminModelPatchBody>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_MODELS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let icon: Option<Option<String>> = match req.icon {
		Some(Some(ref s)) if is_data_uri(s) => match store_from_data_uri(&state.db, s, None, Some("model_icon")).await {
			Ok(stored) => Some(Some(image_url(stored.id))),
			Err(e) => {
				eprintln!("[ADMIN] Failed to store model icon: {e}");
				return ErrorBuilder::new(ErrorCode::InternalError).build();
			}
		},
		Some(ref inner) => Some(inner.as_deref().map(str::to_owned)),
		None => None,
	};

	let mut tx = match state.db.begin().await {
		Ok(tx) => tx,
		Err(e) => {
			eprintln!("[ADMIN] Failed to begin tx: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	if req.display_name.is_some() || req.is_enabled.is_some() {
		match Model::patch(&mut *tx, &id, req.display_name.as_deref(), req.is_enabled).await {
			Ok(Some(_)) => {}
			Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
			Err(e) => {
				eprintln!("[ADMIN] Failed to update model: {e}");
				return ErrorBuilder::new(ErrorCode::InternalError).build();
			}
		}
	}

	let model_info = match Model::find_name_and_model_id(&mut *tx, &id).await {
		Ok(Some(r)) => r,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[ADMIN] Failed to fetch model info: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let (m_name, m_id) = model_info;
	if let Err(e) = ModelConfig::ensure_system_config(&mut *tx, &id, &m_id, &m_name).await {
		eprintln!("[ADMIN] Failed to ensure model config: {e}");
		return ErrorBuilder::new(ErrorCode::InternalError).build();
	}

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
		if let Err(e) = ModelConfig::patch_system_config(&mut *tx, &id, &config_fields).await {
			eprintln!("[ADMIN] Failed to patch model config: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	if let Err(e) = tx.commit().await {
		eprintln!("[ADMIN] Failed to commit tx: {e}");
		return ErrorBuilder::new(ErrorCode::InternalError).build();
	}

	ResponseBuilder::<()>::new(ResponseBody::Empty).build()
}
