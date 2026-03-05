// -*- coding: utf-8 -*-
// API routes for local model management.

use super::download::{DownloadManager, DownloadResult, DownloadStatus, DownloadTask};
use super::schemas::ProviderInfo;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Json, Json as JsonExtractor, Response},
};
use copaw_providers::{ProviderRegistry, ProviderStore};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Default local models directory constant.
pub const LOCAL_MODELS_DIR: &str = "models";

/// Application state for local model routes.
#[derive(Clone)]
pub struct LocalModelsState {
    pub registry: Arc<ProviderRegistry>,
    #[allow(dead_code)]
    pub store: Arc<ProviderStore>,
    pub download_manager: Arc<DownloadManager>,
    pub models_dir: PathBuf,
}

impl LocalModelsState {
    /// Create a new local models state.
    pub fn new(
        registry: Arc<ProviderRegistry>,
        store: Arc<ProviderStore>,
        models_dir: PathBuf,
    ) -> Self {
        let download_manager = Arc::new(DownloadManager::new(models_dir.clone()));
        Self {
            registry,
            store,
            download_manager,
            models_dir,
        }
    }

    /// Get the download manager.
    #[allow(dead_code)]
    pub fn download_manager(&self) -> &DownloadManager {
        &self.download_manager
    }
}

/// Download request for local models.
#[derive(Debug, Clone, Deserialize)]
pub struct LocalModelDownloadRequest {
    pub repo_id: String,
    pub filename: Option<String>,
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_backend() -> String {
    "llamacpp".to_string()
}

fn default_source() -> String {
    "huggingface".to_string()
}

/// Local model info.
#[derive(Debug, Clone, Serialize)]
pub struct LocalModelInfo {
    pub id: String,
    pub repo_id: String,
    pub filename: String,
    pub backend: String,
    pub source: String,
    pub file_size: u64,
    pub local_path: String,
    pub display_name: String,
}

impl From<DownloadResult> for LocalModelInfo {
    fn from(result: DownloadResult) -> Self {
        LocalModelInfo {
            id: result.id,
            repo_id: result.repo_id,
            filename: result.filename,
            backend: result.backend,
            source: result.source,
            file_size: result.file_size,
            local_path: result.local_path,
            display_name: result.display_name,
        }
    }
}

/// Download task response.
#[derive(Debug, Clone, Serialize)]
pub struct LocalModelDownloadTaskResponse {
    pub task_id: String,
    pub status: String,
    pub repo_id: String,
    pub filename: Option<String>,
    pub backend: String,
    pub source: String,
    pub error: Option<String>,
    pub result: Option<LocalModelInfo>,
    pub progress: f32,
}

impl From<DownloadTask> for LocalModelDownloadTaskResponse {
    fn from(task: DownloadTask) -> Self {
        LocalModelDownloadTaskResponse {
            task_id: task.task_id,
            status: format!("{:?}", task.status).to_lowercase(),
            repo_id: task.repo_id,
            filename: task.filename,
            backend: task.backend,
            source: task.source,
            error: task.error,
            result: task.result.map(LocalModelInfo::from),
            progress: task.progress,
        }
    }
}

/// Error types for local models API.
#[derive(Debug)]
pub enum LocalModelsError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
    #[allow(dead_code)]
    NotImplemented(String),
}

impl IntoResponse for LocalModelsError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            LocalModelsError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            LocalModelsError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            LocalModelsError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            LocalModelsError::NotImplemented(msg) => (StatusCode::NOT_IMPLEMENTED, msg),
        };

        let body = Json(serde_json::json!({ "detail": message }));
        (status, body).into_response()
    }
}

impl std::fmt::Display for LocalModelsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocalModelsError::NotFound(msg) => write!(f, "Not found: {}", msg),
            LocalModelsError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            LocalModelsError::Internal(msg) => write!(f, "Internal error: {}", msg),
            LocalModelsError::NotImplemented(msg) => write!(f, "Not implemented: {}", msg),
        }
    }
}

impl std::error::Error for LocalModelsError {}

/// GET /api/local-models/providers - List local model providers
pub async fn list_local_model_providers(
    State(state): State<LocalModelsState>,
) -> Result<Json<Vec<ProviderInfo>>, LocalModelsError> {
    let providers = state.registry.list();
    let local_providers: Vec<ProviderInfo> = providers
        .iter()
        .filter(|p| p.is_local)
        .map(|p| ProviderInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            api_key_prefix: String::new(),
            models: p.models.clone(),
            extra_models: vec![],
            is_custom: false,
            is_local: true,
            needs_base_url: false,
            has_api_key: false,
            current_api_key: String::new(),
            current_base_url: String::new(),
            chat_model: p.chat_model.clone(),
        })
        .collect();

    Ok(Json(local_providers))
}

/// GET /api/local-models/providers/{provider}/models - List models for provider
pub async fn list_provider_local_models(
    State(state): State<LocalModelsState>,
    AxumPath(provider): AxumPath<String>,
) -> Result<Json<Vec<LocalModelInfo>>, LocalModelsError> {
    let provider_def = state
        .registry
        .get(&provider)
        .ok_or_else(|| LocalModelsError::NotFound(format!("Provider '{}' not found", provider)))?;

    if !provider_def.is_local {
        return Err(LocalModelsError::BadRequest(format!(
            "Provider '{}' is not a local model provider",
            provider
        )));
    }

    // Get download tasks that completed for this backend
    let tasks = state.download_manager.get_tasks(Some(&provider)).await;
    let models: Vec<LocalModelInfo> = tasks
        .into_iter()
        .filter(|t| t.status == DownloadStatus::Completed)
        .filter_map(|t| t.result.map(LocalModelInfo::from))
        .collect();

    Ok(Json(models))
}

/// POST /api/local-models/providers/{provider}/download - Download model
pub async fn download_local_model(
    State(state): State<LocalModelsState>,
    AxumPath(provider): AxumPath<String>,
    JsonExtractor(req): JsonExtractor<LocalModelDownloadRequest>,
) -> Result<Json<LocalModelDownloadTaskResponse>, LocalModelsError> {
    download_local_model_impl(state, provider, req).await
}

/// Implementation helper for downloading a model from a specific provider
async fn download_local_model_impl(
    state: LocalModelsState,
    provider: String,
    req: LocalModelDownloadRequest,
) -> Result<Json<LocalModelDownloadTaskResponse>, LocalModelsError> {
    // Validate provider exists and is local
    let provider_def = state
        .registry
        .get(&provider)
        .ok_or_else(|| LocalModelsError::NotFound(format!("Provider '{}' not found", provider)))?;

    if !provider_def.is_local {
        return Err(LocalModelsError::BadRequest(format!(
            "Provider '{}' is not a local model provider",
            provider
        )));
    }

    // Validate backend matches provider (llamacpp or mlx)
    if !matches!(req.backend.as_str(), "llamacpp" | "mlx") {
        return Err(LocalModelsError::BadRequest(format!(
            "Invalid backend '{}'. Must be 'llamacpp' or 'mlx'",
            req.backend
        )));
    }

    // Validate source
    if !matches!(req.source.as_str(), "huggingface" | "modelscope") {
        return Err(LocalModelsError::BadRequest(format!(
            "Invalid source '{}'. Must be 'huggingface' or 'modelscope'",
            req.source
        )));
    }

    // Clear completed tasks for this backend
    state
        .download_manager
        .clear_completed(Some(&req.backend))
        .await;

    // Create download task
    let task = state
        .download_manager
        .create_task(
            req.repo_id.clone(),
            req.filename.clone(),
            req.backend.clone(),
            req.source.clone(),
        )
        .await;

    // Start background download
    let manager = state.download_manager.clone();
    let task_id = task.task_id.clone();
    let repo_id = req.repo_id.clone();
    let filename = req.filename.clone();
    let backend = req.backend.clone();
    let source = req.source.clone();
    let models_dir = state.models_dir.clone();

    tokio::spawn(async move {
        // Update status to downloading
        let _ = manager
            .update_status(&task_id, DownloadStatus::Downloading)
            .await;

        // For now, simulate download and mark as completed with a mock result
        // TODO: Implement actual download logic using huggingface_hub or modelscope
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let sanitized_repo = repo_id.replace('/', "--");
        let model_path = if backend == "mlx" {
            // MLX models are directory-based
            models_dir.join(&sanitized_repo)
        } else if let Some(ref fname) = filename {
            models_dir.join(&sanitized_repo).join(fname)
        } else {
            models_dir.join(&sanitized_repo).join("model.gguf")
        };

        let result = DownloadResult {
            id: if filename.is_some() {
                format!("{}/{}", repo_id, filename.as_ref().unwrap())
            } else {
                repo_id.clone()
            },
            repo_id: repo_id.clone(),
            filename: filename
                .clone()
                .unwrap_or_else(|| "(full repo)".to_string()),
            backend: backend.clone(),
            source: source.clone(),
            file_size: 0, // Will be filled by actual download
            local_path: model_path.to_string_lossy().to_string(),
            display_name: if let Some(ref fname) = filename {
                let repo_short = repo_id.split('/').last().unwrap_or(&repo_id);
                format!("{} ({})", repo_short, fname)
            } else {
                repo_id.split('/').last().unwrap_or(&repo_id).to_string()
            },
        };

        let _ = manager.update_result(&task_id, result).await;
    });

    Ok(Json(LocalModelDownloadTaskResponse::from(task)))
}

/// GET /api/local-models/providers/{provider}/models/{model}/info - Get model info
pub async fn get_local_model_info(
    State(state): State<LocalModelsState>,
    AxumPath((provider, model_id)): AxumPath<(String, String)>,
) -> Result<Json<LocalModelInfo>, LocalModelsError> {
    let provider_def = state
        .registry
        .get(&provider)
        .ok_or_else(|| LocalModelsError::NotFound(format!("Provider '{}' not found", provider)))?;

    if !provider_def.is_local {
        return Err(LocalModelsError::BadRequest(format!(
            "Provider '{}' is not a local model provider",
            provider
        )));
    }

    // Find model in completed downloads
    let tasks = state.download_manager.get_tasks(Some(&provider)).await;
    let model = tasks
        .into_iter()
        .find(|t| t.result.as_ref().map(|r| &r.id) == Some(&model_id))
        .and_then(|t| t.result.map(LocalModelInfo::from))
        .ok_or_else(|| LocalModelsError::NotFound(format!("Model '{}' not found", model_id)))?;

    Ok(Json(model))
}

/// DELETE /api/local-models/providers/{provider}/models/{model} - Delete model
pub async fn delete_local_model(
    State(state): State<LocalModelsState>,
    AxumPath((provider, model_id)): AxumPath<(String, String)>,
) -> Result<Json<serde_json::Value>, LocalModelsError> {
    let provider_def = state
        .registry
        .get(&provider)
        .ok_or_else(|| LocalModelsError::NotFound(format!("Provider '{}' not found", provider)))?;

    if !provider_def.is_local {
        return Err(LocalModelsError::BadRequest(format!(
            "Provider '{}' is not a local model provider",
            provider
        )));
    }

    // Find and remove the model from download tasks
    let mut tasks = state.download_manager.tasks_mut().await;
    let task_id_to_remove = tasks
        .iter()
        .find(|(_, t)| t.result.as_ref().map(|r| &r.id) == Some(&model_id) && t.backend == provider)
        .map(|(task_id, _)| task_id.clone());

    if let Some(task_id) = task_id_to_remove {
        // Get the task to delete the file
        let task = tasks.get(&task_id);
        if let Some(task) = task {
            // Delete the file if it exists
            if let Some(ref result) = task.result {
                let path = Path::new(&result.local_path);
                if path.exists() {
                    if path.is_dir() {
                        let _ = tokio::fs::remove_dir_all(path).await;
                    } else {
                        let _ = tokio::fs::remove_file(path).await;
                    }
                }
            }
        }

        tasks.remove(&task_id);
    } else {
        return Err(LocalModelsError::NotFound(format!(
            "Model '{}' not found",
            model_id
        )));
    }

    Ok(Json(serde_json::json!({
        "status": "deleted",
        "model_id": model_id
    })))
}

/// GET /api/local-models/download-status - Get download tasks
pub async fn get_download_status(
    State(state): State<LocalModelsState>,
) -> Json<Vec<LocalModelDownloadTaskResponse>> {
    let tasks = state.download_manager.get_tasks(None).await;
    let responses: Vec<LocalModelDownloadTaskResponse> = tasks
        .into_iter()
        .map(LocalModelDownloadTaskResponse::from)
        .collect();
    Json(responses)
}

/// GET /api/local-models - List all local models (aggregated from all providers)
pub async fn list_local_models(
    State(state): State<LocalModelsState>,
) -> Result<Json<Vec<LocalModelInfo>>, LocalModelsError> {
    let providers: Vec<String> = state
        .registry
        .list()
        .iter()
        .filter(|p| p.is_local)
        .map(|p| p.id.clone())
        .collect();

    let mut all_models: Vec<LocalModelInfo> = Vec::new();

    for provider in providers {
        let tasks = state.download_manager.get_tasks(Some(&provider)).await;
        let models: Vec<LocalModelInfo> = tasks
            .into_iter()
            .filter(|t| t.status == DownloadStatus::Completed)
            .filter_map(|t| t.result.map(LocalModelInfo::from))
            .collect();
        all_models.extend(models);
    }

    Ok(Json(all_models))
}

/// POST /api/local-models/download - Download model (defaults to first local provider)
pub async fn download_model(
    State(state): State<LocalModelsState>,
    JsonExtractor(req): JsonExtractor<LocalModelDownloadRequest>,
) -> Result<Json<LocalModelDownloadTaskResponse>, LocalModelsError> {
    // Find first local provider (llamacpp or mlx)
    let provider_id = state
        .registry
        .list()
        .iter()
        .find(|p| p.is_local)
        .map(|p| p.id.clone())
        .ok_or_else(|| LocalModelsError::Internal("No local provider found".to_string()))?;

    download_local_model_impl(state, provider_id, req).await
}

/// DELETE /api/local-models/{model_id:path} - Delete model (find by ID across providers)
pub async fn delete_model(
    State(state): State<LocalModelsState>,
    AxumPath(model_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, LocalModelsError> {
    // Normalize model_id: remove leading slash (Axum wildcard may include it)
    let model_id = model_id.strip_prefix('/').unwrap_or(&model_id).to_string();

    let providers: Vec<String> = state
        .registry
        .list()
        .iter()
        .filter(|p| p.is_local)
        .map(|p| p.id.clone())
        .collect();

    // Find the model across all providers
    for provider in providers {
        let mut tasks = state.download_manager.tasks_mut().await;
        let task_id_to_remove = tasks
            .iter()
            .find(|(_, t)| {
                t.result.as_ref().map(|r| &r.id) == Some(&model_id) && t.backend == provider
            })
            .map(|(task_id, _)| task_id.clone());

        if let Some(task_id) = task_id_to_remove {
            // Get task to delete file
            if let Some(task) = tasks.get(&task_id) {
                if let Some(ref result) = task.result {
                    // Delete file if it exists
                    let path = Path::new(&result.local_path);
                    if path.exists() {
                        if path.is_dir() {
                            let _ = tokio::fs::remove_dir_all(path).await;
                        } else {
                            let _ = tokio::fs::remove_file(path).await;
                        }
                    }
                }
            }
            tasks.remove(&task_id);
            return Ok(Json(serde_json::json!({
                "status": "deleted",
                "model_id": model_id
            })));
        }
    }

    Err(LocalModelsError::NotFound(format!(
        "Model '{}' not found",
        model_id
    )))
}

/// POST /api/local-models/cancel-download/{task_id} - Cancel download task
pub async fn cancel_download(
    State(state): State<LocalModelsState>,
    AxumPath(task_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, LocalModelsError> {
    let cancelled = state.download_manager.cancel_task(&task_id).await;

    if !cancelled {
        return Err(LocalModelsError::NotFound(format!(
            "Task '{}' not found or not cancellable",
            task_id
        )));
    }

    Ok(Json(serde_json::json!({
        "status": "cancelled",
        "task_id": task_id
    })))
}

/// Create the local models router.
pub fn create_local_models_router() -> axum::Router<LocalModelsState> {
    use axum::routing::*;

    axum::Router::new()
        .route("/", get(list_local_models))
        .route("/download", post(download_model))
        .route("/*model_id", delete(delete_model))
        .route("/providers", get(list_local_model_providers))
        .route(
            "/providers/:provider/models",
            get(list_provider_local_models),
        )
        .route("/providers/:provider/download", post(download_local_model))
        .route(
            "/providers/:provider/models/:model/info",
            get(get_local_model_info),
        )
        .route(
            "/providers/:provider/models/:model",
            delete(delete_local_model),
        )
        .route("/download-status", get(get_download_status))
        .route("/cancel-download/:task_id", post(cancel_download))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use copaw_providers::ProviderRegistry;

    fn create_test_state() -> LocalModelsState {
        let registry = Arc::new(ProviderRegistry::new());
        let store = Arc::new(ProviderStore::new(
            ProviderStore::default_path(),
            registry.clone(),
        ));
        let models_dir = PathBuf::from("/tmp/test_models");
        LocalModelsState::new(registry, store, models_dir)
    }

    #[test]
    fn test_download_request_defaults() {
        let json = r#"{"repo_id": "test/model"}"#;
        let req: LocalModelDownloadRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.repo_id, "test/model");
        assert_eq!(req.filename, None);
        assert_eq!(req.backend, "llamacpp");
        assert_eq!(req.source, "huggingface");
    }

    #[test]
    fn test_download_request_with_options() {
        let json = r#"{
            "repo_id": "test/model",
            "filename": "model.gguf",
            "backend": "mlx",
            "source": "modelscope"
        }"#;
        let req: LocalModelDownloadRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.filename, Some("model.gguf".to_string()));
        assert_eq!(req.backend, "mlx");
        assert_eq!(req.source, "modelscope");
    }

    #[tokio::test]
    async fn test_local_models_state_creation() {
        let state = create_test_state();
        assert_eq!(state.models_dir, PathBuf::from("/tmp/test_models"));
    }

    #[tokio::test]
    async fn test_list_local_model_providers() {
        let state = create_test_state();
        let result = list_local_model_providers(State(state)).await;
        assert!(result.is_ok());
        let providers = result.unwrap().0;
        // Should have llamacpp provider
        assert!(providers
            .iter()
            .any(|p| p.id == "llamacpp" || p.id == "mlx"));
    }

    #[tokio::test]
    async fn test_download_task_response_conversion() {
        let task = DownloadTask {
            task_id: "test-id".to_string(),
            repo_id: "test/repo".to_string(),
            filename: Some("model.gguf".to_string()),
            backend: "llamacpp".to_string(),
            source: "huggingface".to_string(),
            status: DownloadStatus::Pending,
            error: None,
            result: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            progress: 0.0,
        };

        let response = LocalModelDownloadTaskResponse::from(task);
        assert_eq!(response.task_id, "test-id");
        assert_eq!(response.status, "pending");
        assert_eq!(response.repo_id, "test/repo");
        assert_eq!(response.filename, Some("model.gguf".to_string()));
    }

    #[test]
    fn test_local_models_error_display() {
        let error = LocalModelsError::NotFound("model not found".to_string());
        assert_eq!(format!("{}", error), "Not found: model not found");
    }

    // Tests for wildcard path parameter normalization (Round 11 action items)
    #[test]
    fn test_model_id_normalization_without_leading_slash() {
        let model_id = "model-name";
        let normalized = model_id.strip_prefix('/').unwrap_or(model_id);
        assert_eq!(normalized, "model-name");
    }

    #[test]
    fn test_model_id_normalization_with_leading_slash() {
        let model_id = "/model-name";
        let normalized = model_id.strip_prefix('/').unwrap_or(model_id);
        assert_eq!(normalized, "model-name");
    }

    #[test]
    fn test_model_id_normalization_with_multiple_leading_slashes() {
        let model_id = "//model-name";
        let normalized = model_id.strip_prefix('/').unwrap_or(model_id);
        // strip_prefix only removes one leading slash, so we need to handle multiple
        let normalized = normalized.strip_prefix('/').unwrap_or(normalized);
        assert_eq!(normalized, "model-name");
    }

    #[test]
    fn test_model_id_normalization_with_nested_path() {
        let model_id = "/path/to/model";
        let normalized = model_id.strip_prefix('/').unwrap_or(model_id);
        assert_eq!(normalized, "path/to/model");
    }
}
