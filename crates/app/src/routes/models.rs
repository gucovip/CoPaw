// -*- coding: utf-8 -*-
// API routes for LLM providers and models.

use crate::routes::schemas::{
    AddModelRequest, ActiveModelsInfo, CreateCustomProviderRequest, ModelSlotRequest,
    ProviderConfigRequest, ProviderInfo, TestConnectionResponse,
    TestModelRequest, TestProviderRequest,
};
use copaw_providers::{
    mask_api_key, ModelInfo, ProviderDefinition, ProviderRegistry, ProviderStore,
};
use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

/// Application state for models routes.
#[derive(Clone)]
pub struct ModelsState {
    pub registry: Arc<ProviderRegistry>,
    pub store: Arc<ProviderStore>,
}

/// Trait for types that have a models state.
pub trait HasModelsState {
    fn models_state(&self) -> &ModelsState;
}

/// Build provider info from definition and store data.
fn build_provider_info(
    provider: &ProviderDefinition,
    store: &ProviderStore,
) -> ProviderInfo {
    let data = store.load().unwrap_or_default();

    if provider.is_local {
        return ProviderInfo {
            id: provider.id.clone(),
            name: provider.name.clone(),
            api_key_prefix: String::new(),
            models: provider.models.clone(),
            extra_models: vec![],
            is_custom: false,
            is_local: true,
            needs_base_url: false,
            has_api_key: true,
            current_api_key: String::new(),
            current_base_url: String::new(),
        };
    }

    let (cur_base_url, cur_api_key) = data.get_credentials(&provider.id);
    let configured = data.is_configured(provider);

    let settings = data.providers.get(&provider.id);
    let extra = settings
        .map(|s| s.extra_models.clone())
        .unwrap_or_default();

    let needs_base_url =
        provider.is_custom || provider.default_base_url.is_empty();

    ProviderInfo {
        id: provider.id.clone(),
        name: provider.name.clone(),
        api_key_prefix: provider.api_key_prefix.clone(),
        models: {
            let mut models = provider.models.clone();
            models.extend(extra.clone());
            models
        },
        extra_models: extra,
        is_custom: provider.is_custom,
        is_local: provider.is_local,
        needs_base_url,
        has_api_key: configured,
        current_api_key: mask_api_key(&cur_api_key, 4),
        current_base_url: cur_base_url,
    }
}

/// GET /api/models - List all providers.
pub async fn list_all_providers(
    State(state): State<ModelsState>,
) -> Json<Vec<ProviderInfo>> {
    let providers = state.registry.list();
    let infos: Vec<ProviderInfo> = providers
        .iter()
        .map(|p| build_provider_info(p, &state.store))
        .collect();
    Json(infos)
}

/// PUT /api/models/{provider_id}/config - Configure a provider.
pub async fn configure_provider(
    State(state): State<ModelsState>,
    Path(provider_id): Path<String>,
    Json(body): Json<ProviderConfigRequest>,
) -> Result<Json<ProviderInfo>, ApiError> {
    let provider = state
        .registry
        .get(&provider_id)
        .ok_or_else(|| ApiError::NotFound(format!("Provider '{provider_id}' not found")))?;

    // Allow base_url for custom providers, providers without a default base URL,
    // and Ollama (user may override).
    let allow_base_url = provider.is_custom
        || provider.default_base_url.is_empty()
        || provider.id == "ollama";
    let base_url = if allow_base_url { body.base_url } else { None };

    state
        .store
        .update_settings(&provider_id, body.api_key, base_url)
        .map_err(|e| ApiError::BadRequest(e))?;

    let info = build_provider_info(&provider, &state.store);
    Ok(Json(info))
}

/// POST /api/models/custom-providers - Create a custom provider.
pub async fn create_custom_provider_endpoint(
    State(state): State<ModelsState>,
    Json(body): Json<CreateCustomProviderRequest>,
) -> Result<Json<ProviderInfo>, ApiError> {
    state
        .store
        .create_custom(
            &body.id,
            &body.name,
            &body.default_base_url,
            &body.api_key_prefix,
            body.models,
        )
        .map_err(|e| ApiError::BadRequest(e))?;

    let provider = state
        .registry
        .get(&body.id)
        .ok_or_else(|| ApiError::Internal("Provider not found after creation".to_string()))?;

    let info = build_provider_info(&provider, &state.store);
    Ok(Json(info))
}

/// POST /api/models/{provider_id}/test - Test provider connection.
pub async fn test_provider(
    State(state): State<ModelsState>,
    Path(provider_id): Path<String>,
    Json(body): Json<Option<TestProviderRequest>>,
) -> Result<Json<TestConnectionResponse>, ApiError> {
    let provider = state
        .registry
        .get(&provider_id)
        .ok_or_else(|| ApiError::NotFound(format!("Provider '{provider_id}' not found")))?;

    let (api_key, base_url) = if let Some(req) = body {
        (req.api_key, req.base_url)
    } else {
        let data = state.store.load().map_err(|e| ApiError::Internal(e))?;
        let (url, key) = data.get_credentials(&provider_id);
        (Some(key), Some(url))
    };

    // For now, return a simple test result
    // TODO: Implement actual connection testing in Task 2.5
    if provider.is_local {
        let has_models = !provider.models.is_empty();
        Ok(Json(TestConnectionResponse {
            success: has_models,
            message: if has_models {
                format!("{} is ready with {} model(s).", provider.name, provider.models.len())
            } else {
                format!("{} has no models available.", provider.name)
            },
        }))
    } else if provider_id == "ollama" {
        // TODO: Check Ollama daemon connectivity
        Ok(Json(TestConnectionResponse {
            success: true,
            message: "Ollama daemon test not yet implemented.".to_string(),
        }))
    } else {
        let data = state.store.load().map_err(|e| ApiError::Internal(e))?;
        let configured = data.is_configured(&provider);

        if api_key.as_ref().map_or(true, |k| k.is_empty())
            && base_url.as_ref().map_or(true, |u| u.is_empty())
            && !configured
        {
            return Ok(Json(TestConnectionResponse {
                success: false,
                message: format!("{} not configured. Please add API key.", provider.name),
            }));
        }

        Ok(Json(TestConnectionResponse {
            success: true,
            message: format!("{} configuration test not yet implemented.", provider.name),
        }))
    }
}

/// POST /api/models/{provider_id}/models/test - Test a specific model.
pub async fn test_model(
    State(_state): State<ModelsState>,
    Path(provider_id): Path<String>,
    Json(body): Json<TestModelRequest>,
) -> Result<Json<TestConnectionResponse>, ApiError> {
    // For now, return a simple test result
    // TODO: Implement actual model testing in Task 2.5
    Ok(Json(TestConnectionResponse {
        success: false,
        message: format!(
            "Model test for '{}/{}' not yet implemented.",
            provider_id, body.model_id
        ),
    }))
}

/// DELETE /api/models/custom-providers/{provider_id} - Delete a custom provider.
pub async fn delete_custom_provider_endpoint(
    State(state): State<ModelsState>,
    Path(provider_id): Path<String>,
) -> Result<Json<Vec<ProviderInfo>>, ApiError> {
    state
        .store
        .delete_custom(&provider_id)
        .map_err(|e| ApiError::BadRequest(e))?;

    let providers = state.registry.list();
    let infos: Vec<ProviderInfo> = providers
        .iter()
        .map(|p| build_provider_info(p, &state.store))
        .collect();
    Ok(Json(infos))
}

/// POST /api/models/{provider_id}/models - Add a model to a provider.
pub async fn add_model_endpoint(
    State(state): State<ModelsState>,
    Path(provider_id): Path<String>,
    Json(body): Json<AddModelRequest>,
) -> Result<Json<ProviderInfo>, ApiError> {
    let model = ModelInfo {
        id: body.id,
        name: body.name,
    };

    state
        .store
        .add_model(&provider_id, model)
        .map_err(|e| ApiError::BadRequest(e))?;

    let provider = state
        .registry
        .get(&provider_id)
        .ok_or_else(|| ApiError::Internal("Provider not found".to_string()))?;

    let info = build_provider_info(&provider, &state.store);
    Ok(Json(info))
}

/// DELETE /api/models/{provider_id}/models/{model_id} - Remove a model from a provider.
pub async fn remove_model_endpoint(
    State(state): State<ModelsState>,
    Path((provider_id, model_id)): Path<(String, String)>,
) -> Result<Json<ProviderInfo>, ApiError> {
    state
        .store
        .remove_model(&provider_id, &model_id)
        .map_err(|e| ApiError::BadRequest(e))?;

    let provider = state
        .registry
        .get(&provider_id)
        .ok_or_else(|| ApiError::Internal("Provider not found".to_string()))?;

    let info = build_provider_info(&provider, &state.store);
    Ok(Json(info))
}

/// GET /api/models/active - Get active LLM.
pub async fn get_active_models(
    State(state): State<ModelsState>,
) -> Json<ActiveModelsInfo> {
    let data = state.store.load().unwrap_or_default();
    Json(ActiveModelsInfo {
        active_llm: (&data.active_llm).into(),
    })
}

/// PUT /api/models/active - Set active LLM.
pub async fn set_active_model(
    State(state): State<ModelsState>,
    Json(body): Json<ModelSlotRequest>,
) -> Result<Json<ActiveModelsInfo>, ApiError> {
    let provider = state
        .registry
        .get(&body.provider_id)
        .ok_or_else(|| ApiError::NotFound(format!("Provider '{}' not found", body.provider_id)))?;

    let data = state.store.load().map_err(|e| ApiError::Internal(e))?;

    if !data.is_configured(&provider) {
        let msg = if provider.is_custom || provider.default_base_url.is_empty() {
            format!(
                "Provider '{}' has no base_url configured. Please configure the base URL first.",
                provider.name
            )
        } else {
            format!(
                "Provider '{}' has no API key configured. Please configure the API key first.",
                provider.name
            )
        };
        return Err(ApiError::BadRequest(msg));
    }

    if body.model.is_empty() {
        return Err(ApiError::BadRequest("Model is required.".to_string()));
    }

    state
        .store
        .set_active_llm(&body.provider_id, &body.model)
        .map_err(|e| ApiError::Internal(e))?;

    let data = state.store.load().map_err(|e| ApiError::Internal(e))?;
    Ok(Json(ActiveModelsInfo {
        active_llm: (&data.active_llm).into(),
    }))
}

/// API error types.
#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({ "detail": message }));
        (status, body).into_response()
    }
}

/// Create the models router.
pub fn create_models_router() -> axum::Router<ModelsState> {
    use axum::routing::*;

    axum::Router::new()
        .route("/", get(list_all_providers))
        .route("/:provider_id/config", put(configure_provider))
        .route("/custom-providers", post(create_custom_provider_endpoint))
        .route("/custom-providers/:provider_id", delete(delete_custom_provider_endpoint))
        .route("/:provider_id/test", post(test_provider))
        .route("/:provider_id/models/test", post(test_model))
        .route("/:provider_id/models", post(add_model_endpoint))
        .route("/:provider_id/models/:model_id", delete(remove_model_endpoint))
        .route("/active", get(get_active_models))
        .route("/active", put(set_active_model))
}

#[cfg(test)]
mod tests {
    use super::*;
    use copaw_providers::{ModelInfo, ProviderDefinition};
    use tempfile::NamedTempFile;

    /// Create a test registry (uses built-in providers)
    fn create_test_registry() -> ProviderRegistry {
        ProviderRegistry::new()
    }

    /// Create a test store with a temporary file
    async fn create_test_store(registry: Arc<ProviderRegistry>) -> ProviderStore {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        let _ = temp_file.into_temp_path();
        ProviderStore::new(path, registry)
    }

    /// Create a test state
    async fn create_test_state() -> ModelsState {
        let registry = Arc::new(create_test_registry());
        let store = Arc::new(create_test_store(registry.clone()).await);
        ModelsState { registry, store }
    }

    #[tokio::test]
    async fn test_list_all_providers() {
        let state = create_test_state().await;
        let result = list_all_providers(State(state)).await;
        let providers = result.0;

        assert!(!providers.is_empty());
        // Should have built-in providers like openai
        assert!(providers.iter().any(|p| p.id == "openai" || p.id == "local-models"));
    }

    #[tokio::test]
    async fn test_configure_provider_not_found() {
        let state = create_test_state().await;
        let body = ProviderConfigRequest {
            api_key: Some("test-key".to_string()),
            base_url: None,
        };

        let result = configure_provider(
            State(state),
            Path("unknown-provider".to_string()),
            Json(body),
        )
        .await;

        assert!(result.is_err());
        if let Err(ApiError::NotFound(msg)) = result {
            assert!(msg.contains("not found"));
        } else {
            panic!("Expected NotFound error");
        }
    }

    #[tokio::test]
    async fn test_configure_provider_success() {
        let state = create_test_state().await;
        let body = ProviderConfigRequest {
            api_key: Some("sk-test123".to_string()),
            base_url: None,
        };

        let result = configure_provider(State(state), Path("openai".to_string()), Json(body)).await;
        assert!(result.is_ok());

        let info = result.unwrap().0;
        assert_eq!(info.id, "openai");
        assert!(info.has_api_key);
    }

    #[tokio::test]
    async fn test_create_custom_provider() {
        let state = create_test_state().await;
        let body = CreateCustomProviderRequest {
            id: "custom-openai".to_string(),
            name: "Custom OpenAI".to_string(),
            default_base_url: "https://custom.openai.com/v1".to_string(),
            api_key_prefix: "sk-".to_string(),
            models: vec![ModelInfo {
                id: "gpt-4o".to_string(),
                name: "GPT-4o".to_string(),
            }],
        };

        let result = create_custom_provider_endpoint(State(state), Json(body)).await;
        assert!(result.is_ok());

        let info = result.unwrap().0;
        assert_eq!(info.id, "custom-openai");
        assert!(info.is_custom);
    }

    #[tokio::test]
    async fn test_test_provider_not_found() {
        let state = create_test_state().await;
        let result = test_provider(
            State(state),
            Path("unknown".to_string()),
            Json(None),
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_test_provider_local() {
        let state = create_test_state().await;
        // Use llamacpp which is a built-in local provider
        let result = test_provider(
            State(state),
            Path("llamacpp".to_string()),
            Json(None),
        )
        .await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        // Local providers with no models return success=false
        assert!(response.message.contains("model") || response.message.contains("Llama.cpp"));
    }

    #[tokio::test]
    async fn test_test_provider_with_credentials() {
        let state = create_test_state().await;
        let body = TestProviderRequest {
            api_key: Some("sk-test".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
        };

        let result = test_provider(
            State(state),
            Path("openai".to_string()),
            Json(Some(body)),
        )
        .await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        // TODO returns true for configured providers
        assert!(response.message.contains("not yet implemented"));
    }

    #[tokio::test]
    async fn test_test_model() {
        let state = create_test_state().await;
        let body = TestModelRequest {
            model_id: "gpt-4o".to_string(),
        };

        let result = test_model(
            State(state),
            Path("openai".to_string()),
            Json(body),
        )
        .await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert!(!response.success);
        assert!(response.message.contains("not yet implemented"));
    }

    #[tokio::test]
    async fn test_delete_custom_provider() {
        let state = create_test_state().await;

        // First create a custom provider
        let body = CreateCustomProviderRequest {
            id: "test-custom".to_string(),
            name: "Test Custom".to_string(),
            default_base_url: "https://test.com".to_string(),
            api_key_prefix: "test-".to_string(),
            models: vec![],
        };
        let _ = create_custom_provider_endpoint(State(state.clone()), Json(body)).await;

        // Then delete it
        let result = delete_custom_provider_endpoint(
            State(state),
            Path("test-custom".to_string()),
        )
        .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_model() {
        let state = create_test_state().await;

        // Add a model to a built-in provider (openai)
        let body = AddModelRequest {
            id: "gpt-4o-mini-test".to_string(),
            name: "GPT-4o Mini Test".to_string(),
        };
        let result = add_model_endpoint(
            State(state),
            Path("openai".to_string()),
            Json(body),
        )
        .await;

        if let Err(e) = &result {
            eprintln!("Error adding model: {:?}", e);
        }
        assert!(result.is_ok());
        let info = result.unwrap().0;
        assert!(info.models.iter().any(|m| m.id == "gpt-4o-mini-test"));
    }

    #[tokio::test]
    async fn test_remove_model() {
        let state = create_test_state().await;

        // Add a model to a built-in provider first
        let add_body = AddModelRequest {
            id: "test-model-remove".to_string(),
            name: "Test Model Remove".to_string(),
        };
        let add_result = add_model_endpoint(
            State(state.clone()),
            Path("openai".to_string()),
            Json(add_body),
        )
        .await;

        if let Err(e) = &add_result {
            eprintln!("Error adding model: {:?}", e);
        }
        assert!(add_result.is_ok());

        // Then remove the model
        let result = remove_model_endpoint(
            State(state),
            Path(("openai".to_string(), "test-model-remove".to_string())),
        )
        .await;

        if let Err(e) = &result {
            eprintln!("Error removing model: {:?}", e);
        }
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_active_models() {
        let state = create_test_state().await;
        let result = get_active_models(State(state)).await;

        let info = result.0;
        // Initially empty, but the struct should exist
        assert_eq!(info.active_llm.provider_id, "");
        assert_eq!(info.active_llm.model, "");
    }

    #[tokio::test]
    async fn test_set_active_model_not_found() {
        let state = create_test_state().await;
        let body = ModelSlotRequest {
            provider_id: "unknown".to_string(),
            model: "test-model".to_string(),
        };

        let result = set_active_model(State(state), Json(body)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_active_model_empty_model() {
        let state = create_test_state().await;
        let body = ModelSlotRequest {
            provider_id: "openai".to_string(),
            model: String::new(),
        };

        let result = set_active_model(State(state), Json(body)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_api_error_not_found() {
        let error = ApiError::NotFound("test".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_api_error_bad_request() {
        let error = ApiError::BadRequest("test".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_api_error_internal() {
        let error = ApiError::Internal("test".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_build_provider_info_local() {
        let registry = Arc::new(create_test_registry());
        let store = Arc::new(create_test_store(registry.clone()).await);

        let provider = registry.get("llamacpp").unwrap();
        let info = build_provider_info(&provider, &store);

        assert_eq!(info.id, "llamacpp");
        assert!(info.is_local);
        assert!(info.has_api_key);
    }

    #[tokio::test]
    async fn test_build_provider_info_remote() {
        let registry = Arc::new(create_test_registry());
        let store = Arc::new(create_test_store(registry.clone()).await);

        let provider = registry.get("openai").unwrap();
        let info = build_provider_info(&provider, &store);

        assert_eq!(info.id, "openai");
        assert!(!info.is_local);
        assert!(!info.has_api_key); // No API key configured
    }

    #[tokio::test]
    async fn test_create_models_router() {
        let _router = create_models_router();
        // Router was created successfully
        // No direct way to inspect routes in Axum 0.7
    }
}
