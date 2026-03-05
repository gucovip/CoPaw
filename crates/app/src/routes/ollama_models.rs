// -*- coding: utf-8 -*-
// API routes for Ollama model management.

use super::download::{DownloadManager, DownloadStatus, DownloadTask};
use super::schemas::ProviderInfo;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Json, Json as JsonExtractor, Response},
};
use copaw_providers::{ProviderRegistry, ProviderStore};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Application state for Ollama model routes.
#[derive(Clone)]
pub struct OllamaModelsState {
    pub registry: Arc<ProviderRegistry>,
    #[allow(dead_code)]
    pub store: Arc<ProviderStore>,
    pub download_manager: Arc<DownloadManager>,
}

impl OllamaModelsState {
    /// Create a new Ollama models state.
    pub fn new(
        registry: Arc<ProviderRegistry>,
        store: Arc<ProviderStore>,
        download_manager: Arc<DownloadManager>,
    ) -> Self {
        Self {
            registry,
            store,
            download_manager,
        }
    }
}

/// Ollama download request.
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaDownloadRequest {
    pub name: String,
}

/// Ollama model info.
#[derive(Debug, Clone, Serialize)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: u64,
    pub digest: Option<String>,
    pub modified_at: Option<String>,
}

/// Ollama download task response.
#[derive(Debug, Clone, Serialize)]
pub struct OllamaDownloadTaskResponse {
    pub task_id: String,
    pub status: String,
    pub name: String,
    pub error: Option<String>,
    pub result: Option<OllamaModelInfo>,
    pub progress: f32,
}

impl From<DownloadTask> for OllamaDownloadTaskResponse {
    fn from(task: DownloadTask) -> Self {
        let name = task.repo_id.clone(); // Store model name in repo_id
        OllamaDownloadTaskResponse {
            task_id: task.task_id,
            status: format!("{:?}", task.status).to_lowercase(),
            name,
            error: task.error,
            result: task.result.map(|r| OllamaModelInfo {
                name: r.repo_id,
                size: r.file_size,
                digest: None,
                modified_at: None,
            }),
            progress: task.progress,
        }
    }
}

/// Error types for Ollama models API.
#[derive(Debug)]
pub enum OllamaModelsError {
    NotFound(String),
    #[allow(dead_code)]
    BadRequest(String),
    #[allow(dead_code)]
    Internal(String),
    NotImplemented(String),
    #[allow(dead_code)]
    ServiceUnavailable(String),
}

impl IntoResponse for OllamaModelsError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            OllamaModelsError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            OllamaModelsError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            OllamaModelsError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            OllamaModelsError::NotImplemented(msg) => (StatusCode::NOT_IMPLEMENTED, msg),
            OllamaModelsError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
        };

        let body = Json(serde_json::json!({ "detail": message }));
        (status, body).into_response()
    }
}

impl std::fmt::Display for OllamaModelsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OllamaModelsError::NotFound(msg) => write!(f, "Not found: {}", msg),
            OllamaModelsError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            OllamaModelsError::Internal(msg) => write!(f, "Internal error: {}", msg),
            OllamaModelsError::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
            OllamaModelsError::ServiceUnavailable(msg) => write!(f, "Service unavailable: {}", msg),
        }
    }
}

impl std::error::Error for OllamaModelsError {}

/// GET /api/ollama-models - List all Ollama models
pub async fn list_ollama_models(
    State(state): State<OllamaModelsState>,
) -> Result<Json<Vec<OllamaModelInfo>>, OllamaModelsError> {
    list_ollama_models_impl(state, "ollama".to_string()).await
}

/// POST /api/ollama-models/download - Pull from Ollama
pub async fn download_ollama_model(
    State(state): State<OllamaModelsState>,
    JsonExtractor(req): JsonExtractor<OllamaDownloadRequest>,
) -> Result<Json<OllamaDownloadTaskResponse>, OllamaModelsError> {
    pull_ollama_model_impl(state, "ollama".to_string(), req).await
}

/// DELETE /api/ollama-models/{name:path} - Delete Ollama model
pub async fn delete_ollama_model_flat(
    State(state): State<OllamaModelsState>,
    AxumPath(model_name): AxumPath<String>,
) -> Result<Json<serde_json::Value>, OllamaModelsError> {
    // Normalize model_name: remove leading slash (Axum wildcard may include it)
    let model_name = model_name
        .strip_prefix('/')
        .unwrap_or(&model_name)
        .to_string();
    delete_ollama_model_impl(state, model_name).await
}

/// GET /api/ollama-models/providers - List Ollama providers
pub async fn list_ollama_providers(
    State(state): State<OllamaModelsState>,
) -> Result<Json<Vec<ProviderInfo>>, OllamaModelsError> {
    let ollama_provider = state.registry.get("ollama");

    match ollama_provider {
        Some(provider) => {
            let info = ProviderInfo {
                id: provider.id.clone(),
                name: provider.name.clone(),
                api_key_prefix: String::new(),
                models: provider.models.clone(),
                extra_models: vec![],
                is_custom: false,
                is_local: true,
                needs_base_url: false,
                has_api_key: false,
                current_api_key: String::new(),
                current_base_url: String::new(),
                chat_model: provider.chat_model.clone(),
            };
            Ok(Json(vec![info]))
        }
        None => {
            // Ollama SDK not available
            Err(OllamaModelsError::NotImplemented(
                "Ollama SDK not configured".to_string(),
            ))
        }
    }
}

/// GET /api/ollama-models/providers/{provider}/models - List Ollama models
pub async fn list_provider_ollama_models(
    State(state): State<OllamaModelsState>,
    AxumPath(_provider): AxumPath<String>,
) -> Result<Json<Vec<OllamaModelInfo>>, OllamaModelsError> {
    let _provider = _provider; // suppress unused warning
                               // Get completed download tasks for Ollama
    let tasks = state.download_manager.get_tasks(Some("ollama")).await;
    let models: Vec<OllamaModelInfo> = tasks
        .into_iter()
        .filter(|t| t.status == DownloadStatus::Completed)
        .filter_map(|t| {
            t.result.map(|r| OllamaModelInfo {
                name: r.repo_id,
                size: r.file_size,
                digest: None,
                modified_at: None,
            })
        })
        .collect();

    Ok(Json(models))
}

/// POST /api/ollama-models/providers/{provider}/pull - Pull Ollama model
pub async fn pull_ollama_model(
    State(state): State<OllamaModelsState>,
    AxumPath(_provider): AxumPath<String>,
    JsonExtractor(req): JsonExtractor<OllamaDownloadRequest>,
) -> Result<Json<OllamaDownloadTaskResponse>, OllamaModelsError> {
    let _provider = _provider; // suppress unused warning
                               // Clear completed tasks
    state.download_manager.clear_completed(Some("ollama")).await;

    // Create download task (store model name in repo_id)
    let task = state
        .download_manager
        .create_task(
            req.name.clone(),
            None,
            "ollama".to_string(),
            "ollama".to_string(),
        )
        .await;

    // Start background pull
    let manager = state.download_manager.clone();
    let task_id = task.task_id.clone();
    let model_name = req.name.clone();

    tokio::spawn(async move {
        // Update status to downloading
        let _ = manager
            .update_status(&task_id, DownloadStatus::Downloading)
            .await;

        // For now, simulate pull and mark as completed
        // TODO: Implement actual Ollama API call
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        use super::download::DownloadResult;

        let result = DownloadResult {
            id: model_name.clone(),
            repo_id: model_name.clone(),
            filename: model_name.clone(),
            backend: "ollama".to_string(),
            source: "ollama".to_string(),
            file_size: 0,              // Will be filled by actual pull
            local_path: String::new(), // Ollama manages its own storage
            display_name: model_name.clone(),
        };

        let _ = manager.update_result(&task_id, result).await;
    });

    Ok(Json(OllamaDownloadTaskResponse::from(task)))
}

/// GET /api/ollama-models/providers/{provider}/models/{model}/info - Get model info
pub async fn get_ollama_model_info(
    State(state): State<OllamaModelsState>,
    AxumPath((_provider, model_name)): AxumPath<(String, String)>,
) -> Result<Json<OllamaModelInfo>, OllamaModelsError> {
    let _provider = _provider; // suppress unused warning
                               // Find model in completed downloads
    let tasks = state.download_manager.get_tasks(Some("ollama")).await;
    let model = tasks
        .into_iter()
        .find(|t| t.result.as_ref().map(|r| &r.id) == Some(&model_name))
        .and_then(|t| {
            t.result.map(|r| OllamaModelInfo {
                name: r.repo_id,
                size: r.file_size,
                digest: None,
                modified_at: None,
            })
        })
        .ok_or_else(|| OllamaModelsError::NotFound(format!("Model '{}' not found", model_name)))?;

    Ok(Json(model))
}

/// DELETE /api/ollama-models/providers/{provider}/models/{model} - Delete Ollama model
pub async fn delete_ollama_model(
    State(state): State<OllamaModelsState>,
    AxumPath((_provider, model_name)): AxumPath<(String, String)>,
) -> Result<Json<serde_json::Value>, OllamaModelsError> {
    // Find and remove the model from download tasks
    let mut tasks = state.download_manager.tasks_mut().await;
    let task_id_to_remove = tasks
        .iter()
        .find(|(_, t)| t.result.as_ref().map(|r| &r.id) == Some(&model_name))
        .map(|(task_id, _)| task_id.clone());

    if let Some(task_id) = task_id_to_remove {
        // TODO: Call Ollama API to actually delete the model
        tasks.remove(&task_id);
    } else {
        return Err(OllamaModelsError::NotFound(format!(
            "Model '{}' not found",
            model_name
        )));
    }

    Ok(Json(serde_json::json!({
        "status": "deleted",
        "name": model_name
    })))
}

/// GET /api/ollama-models/download-status - Get download tasks
pub async fn get_ollama_download_status(
    State(state): State<OllamaModelsState>,
) -> Json<Vec<OllamaDownloadTaskResponse>> {
    let tasks = state.download_manager.get_tasks(Some("ollama")).await;
    let responses: Vec<OllamaDownloadTaskResponse> = tasks
        .into_iter()
        .map(OllamaDownloadTaskResponse::from)
        .collect();
    Json(responses)
}

/// DELETE /api/ollama-models/download/{task_id} - Cancel download task
pub async fn cancel_ollama_download(
    State(state): State<OllamaModelsState>,
    AxumPath(task_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, OllamaModelsError> {
    let cancelled = state.download_manager.cancel_task(&task_id).await;

    if !cancelled {
        return Err(OllamaModelsError::NotFound(format!(
            "Task '{}' not found or not cancellable",
            task_id
        )));
    }

    Ok(Json(serde_json::json!({
        "status": "cancelled",
        "task_id": task_id
    })))
}

/// Implementation helper for listing Ollama models from a provider
async fn list_ollama_models_impl(
    state: OllamaModelsState,
    provider: String,
) -> Result<Json<Vec<OllamaModelInfo>>, OllamaModelsError> {
    let _provider = provider;
    // Get completed download tasks for Ollama
    let tasks = state.download_manager.get_tasks(Some("ollama")).await;
    let models: Vec<OllamaModelInfo> = tasks
        .into_iter()
        .filter(|t| t.status == DownloadStatus::Completed)
        .filter_map(|t| {
            t.result.map(|r| OllamaModelInfo {
                name: r.repo_id,
                size: r.file_size,
                digest: None,
                modified_at: None,
            })
        })
        .collect();

    Ok(Json(models))
}

/// Implementation helper for pulling from a specific provider
async fn pull_ollama_model_impl(
    state: OllamaModelsState,
    provider: String,
    req: OllamaDownloadRequest,
) -> Result<Json<OllamaDownloadTaskResponse>, OllamaModelsError> {
    let _provider = provider;
    // Clear completed tasks
    state.download_manager.clear_completed(Some("ollama")).await;

    // Create download task (store model name in repo_id)
    let task = state
        .download_manager
        .create_task(
            req.name.clone(),
            None,
            "ollama".to_string(),
            "ollama".to_string(),
        )
        .await;

    // Start background pull
    let manager = state.download_manager.clone();
    let task_id = task.task_id.clone();
    let model_name = req.name.clone();

    tokio::spawn(async move {
        // Update status to downloading
        let _ = manager
            .update_status(&task_id, DownloadStatus::Downloading)
            .await;

        // For now, simulate pull and mark as completed
        // TODO: Implement actual Ollama API call
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        use super::download::DownloadResult;

        let result = DownloadResult {
            id: model_name.clone(),
            repo_id: model_name.clone(),
            filename: model_name.clone(),
            backend: "ollama".to_string(),
            source: "ollama".to_string(),
            file_size: 0,              // Will be filled by actual pull
            local_path: String::new(), // Ollama manages its own storage
            display_name: model_name.clone(),
        };

        let _ = manager.update_result(&task_id, result).await;
    });

    Ok(Json(OllamaDownloadTaskResponse::from(task)))
}

/// Implementation helper for deleting an Ollama model
async fn delete_ollama_model_impl(
    state: OllamaModelsState,
    model_name: String,
) -> Result<Json<serde_json::Value>, OllamaModelsError> {
    // Find and remove the model from download tasks
    let mut tasks = state.download_manager.tasks_mut().await;
    let task_id_to_remove = tasks
        .iter()
        .find(|(_, t)| t.result.as_ref().map(|r| &r.id) == Some(&model_name))
        .map(|(task_id, _)| task_id.clone());

    if let Some(task_id) = task_id_to_remove {
        // TODO: Call Ollama API to actually delete the model
        tasks.remove(&task_id);
        return Ok(Json(serde_json::json!({
            "status": "deleted",
            "name": model_name
        })));
    }

    Err(OllamaModelsError::NotFound(format!(
        "Model '{}' not found",
        model_name
    )))
}

/// Create the Ollama models router.
pub fn create_ollama_models_router() -> axum::Router<OllamaModelsState> {
    use axum::routing::*;

    axum::Router::new()
        .route("/", get(list_ollama_models))
        .route("/download", post(download_ollama_model))
        .route("/*name", delete(delete_ollama_model_flat))
        .route("/providers", get(list_ollama_providers))
        .route(
            "/providers/:provider/models",
            get(list_provider_ollama_models),
        )
        .route("/providers/:provider/pull", post(pull_ollama_model))
        .route(
            "/providers/:provider/models/:model/info",
            get(get_ollama_model_info),
        )
        .route(
            "/providers/:provider/models/:model",
            delete(delete_ollama_model),
        )
        .route("/download-status", get(get_ollama_download_status))
        .route("/download/:task_id", delete(cancel_ollama_download))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::download::DownloadManager;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn create_test_state() -> OllamaModelsState {
        let registry = Arc::new(ProviderRegistry::new());
        let store = Arc::new(ProviderStore::new(
            ProviderStore::default_path(),
            registry.clone(),
        ));
        let download_manager = Arc::new(DownloadManager::new(PathBuf::from("/tmp/test_models")));
        OllamaModelsState::new(registry, store, download_manager)
    }

    #[test]
    fn test_ollama_download_request() {
        let json = r#"{"name": "llama3:8b"}"#;
        let req: OllamaDownloadRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "llama3:8b");
    }

    #[test]
    fn test_ollama_model_info_serialization() {
        let info = OllamaModelInfo {
            name: "llama3:8b".to_string(),
            size: 4096000000,
            digest: Some("abc123".to_string()),
            modified_at: Some("2024-01-01".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("llama3:8b"));
        assert!(json.contains("abc123"));
    }

    #[tokio::test]
    async fn test_ollama_models_state_creation() {
        let state = create_test_state();
        assert!(Arc::strong_count(&state.registry) >= 1);
    }

    #[test]
    fn test_ollama_models_error_display() {
        let error = OllamaModelsError::NotFound("model not found".to_string());
        assert_eq!(format!("{}", error), "Not found: model not found");
    }

    #[test]
    fn test_ollama_models_error_service_unavailable() {
        let error = OllamaModelsError::ServiceUnavailable("Ollama daemon not running".to_string());
        assert!(format!("{}", error).contains("Service unavailable"));
    }

    // Tests for wildcard path parameter normalization (Round 11 action items)
    #[test]
    fn test_model_name_normalization_without_leading_slash() {
        let model_name = "llama3:8b";
        let normalized = model_name.strip_prefix('/').unwrap_or(model_name);
        assert_eq!(normalized, "llama3:8b");
    }

    #[test]
    fn test_model_name_normalization_with_leading_slash() {
        let model_name = "/llama3:8b";
        let normalized = model_name.strip_prefix('/').unwrap_or(model_name);
        assert_eq!(normalized, "llama3:8b");
    }

    #[test]
    fn test_model_name_normalization_with_nested_path() {
        let model_name = "/org/model";
        let normalized = model_name.strip_prefix('/').unwrap_or(model_name);
        assert_eq!(normalized, "org/model");
    }
}
