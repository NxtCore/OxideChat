use crate::routes::public::auth::get_current_user;
use crate::types::axum::{AdminModelUpdateBody, ModelListParams};
use crate::types::models::Model;
use crate::types::models_configs::ModelConfig;
use crate::types::{BaseType, JobState};
use crate::utils::images::{image_url, is_data_uri, store_from_data_uri};
use crate::utils::response::{ErrorBuilder, ErrorCode, ResponseBody, ResponseBuilder};
use axum::extract::Query;
use axum::{
	Json,
	extract::{Path, State},
	response::IntoResponse,
};
use std::sync::Arc;
use tower_cookies::Cookies;
use uuid::Uuid;

const ADMIN_MODELS_VIEW: &str = "admin.providers.view";
const ADMIN_MODELS_EDIT: &str = "admin.providers.edit";

fn build_extra_settings(base: Option<&serde_json::Value>, reasoning_effort: Option<&str>, reasoning_budget_tokens: Option<u32>) -> Option<serde_json::Value> {
	let mut map = if let Some(serde_json::Value::Object(obj)) = base {
		obj.clone()
	} else {
		serde_json::Map::new()
	};

	match reasoning_effort {
		Some(effort) => {
			map.insert("reasoning_effort".to_string(), serde_json::Value::String(effort.to_string()));
		}
		None => {
			map.remove("reasoning_effort");
		}
	}

	match reasoning_budget_tokens {
		Some(budget) => {
			map.insert("reasoning_budget_tokens".to_string(), serde_json::Value::Number(budget.into()));
		}
		None => {
			map.remove("reasoning_budget_tokens");
		}
	}

	if map.is_empty() { None } else { Some(serde_json::Value::Object(map)) }
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

	let models = match Model::list_paginated_admin(&state.db, params.page.unwrap_or(1), params.size.unwrap_or(0), params.query.clone()).await {
		Ok(m) => m,
		Err(e) => {
			eprintln!("[ADMIN] Failed to list models: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	ResponseBuilder::new(ResponseBody::Json(models)).build()
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

	let row = match Model::find_by_id_with_config(&state.db, &id).await {
		Ok(Some(r)) => r,
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(e) => {
			eprintln!("[ADMIN] Failed to get model: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	ResponseBuilder::new(ResponseBody::Json(row)).build()
}

/// PUT /api/v1/admin/models/:id
pub async fn update_model(State(state): State<Arc<JobState>>, cookies: Cookies, Path(id): Path<Uuid>, Json(req): Json<AdminModelUpdateBody>) -> impl IntoResponse {
	let user = match get_current_user(&state.db, &cookies).await {
		Some(user) => user,
		None => return ErrorBuilder::new(ErrorCode::NotAuthenticated).build(),
	};

	if !user.has_permission(&state.db, ADMIN_MODELS_EDIT).await {
		return ErrorBuilder::new(ErrorCode::InsufficientPermissions).build();
	}

	let icon: Option<String> = match &req.icon {
		Some(s) if is_data_uri(s) => match store_from_data_uri(&state.db, s, None, Some("model_icon")).await {
			Ok(stored) => Some(image_url(stored.id)),
			Err(e) => {
				eprintln!("[ADMIN] Failed to store model icon: {e}");
				return ErrorBuilder::new(ErrorCode::InternalError).build();
			}
		},
		Some(s) => Some(s.clone()),
		None => None,
	};

	let mut tx = match state.db.begin().await {
		Ok(tx) => tx,
		Err(e) => {
			eprintln!("[ADMIN] Failed to begin tx: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	};

	let model_info: Option<(String, String)> = match sqlx::query_as(format!("SELECT display_name, model_id FROM {} WHERE id = $1", Model::new().table()).as_str())
		.bind(id)
		.fetch_optional(&mut *tx)
		.await
	{
		Ok(Some(r)) => Some(r),
		Ok(None) => return ErrorBuilder::new(ErrorCode::NotFound).build(),
		Err(_) => None,
	};

	if let Some((m_name, m_id)) = model_info {
		let extra_settings = build_extra_settings(req.extra_settings.as_ref(), req.reasoning_effort.as_deref(), req.reasoning_budget_tokens);

		if let Err(e) = ModelConfig::upsert_system_config(
			&mut *tx,
			&id,
			&m_id,
			&m_name,
			req.description.as_deref(),
			icon.as_deref(),
			req.system_prompt.as_deref(),
			req.sampling.as_ref(),
			req.input_modalities.as_ref(),
			req.output_modalities.as_ref(),
			req.context_length,
			req.max_output_tokens,
			req.enabled_tools.as_ref(),
			req.is_public,
			req.is_featured,
			req.is_default,
			req.is_favorite,
			req.category.as_deref(),
			req.tags.as_ref(),
			extra_settings.as_ref(),
		)
		.await
		{
			eprintln!("[ADMIN] Failed to upsert model config: {e}");
			return ErrorBuilder::new(ErrorCode::InternalError).build();
		}
	}

	if let Err(e) = tx.commit().await {
		eprintln!("[ADMIN] Failed to commit tx: {e}");
		return ErrorBuilder::new(ErrorCode::InternalError).build();
	}

	ResponseBuilder::<()>::new(ResponseBody::Empty).build()
}
