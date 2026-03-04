//! Agent API routes.
//!
//! Provides REST endpoints for agent file and configuration management.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use copaw_agents::AgentFileManager;
use copaw_config::{load_config, save_config, CoPawConfig};
use std::sync::Arc;

/// State for agent routes.
#[derive(Clone)]
pub struct AgentState {
    pub file_manager: Arc<AgentFileManager>,
}

/// Error response for agent operations.
#[derive(Debug)]
pub enum AgentRouteError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl axum::response::IntoResponse for AgentRouteError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AgentRouteError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AgentRouteError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AgentRouteError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({ "detail": message }));
        (status, body).into_response()
    }
}

impl std::fmt::Display for AgentRouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRouteError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AgentRouteError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            AgentRouteError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AgentRouteError {}

/// GET /api/agent/files - List working files.
pub async fn list_working_files(
    State(state): State<AgentState>,
) -> Result<Json<Vec<crate::routes::schemas::MdFileInfo>>, AgentRouteError> {
    let files = state
        .file_manager
        .list_working_mds()
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    let file_infos: Vec<crate::routes::schemas::MdFileInfo> = files
        .into_iter()
        .map(|f| crate::routes::schemas::MdFileInfo {
            filename: f.filename,
            path: f.path.display().to_string(),
            size: f.size,
            created_time: f.created_time,
            modified_time: f.modified_time,
        })
        .collect();

    Ok(Json(file_infos))
}

/// GET /api/agent/files/:name - Read a working file.
pub async fn read_working_file(
    State(state): State<AgentState>,
    Path(md_name): Path<String>,
) -> Result<Json<crate::routes::schemas::MdFileContent>, AgentRouteError> {
    let content = state
        .file_manager
        .read_working_md(&md_name)
        .map_err(|e| match e {
            copaw_agents::AgentFileManagerError::FileNotFound(_) => {
                AgentRouteError::NotFound(format!("File not found: {}", md_name))
            }
            _ => AgentRouteError::Internal(e.to_string()),
        })?;

    Ok(Json(crate::routes::schemas::MdFileContent { content }))
}

/// PUT /api/agent/files/:name - Write a working file.
pub async fn write_working_file(
    State(state): State<AgentState>,
    Path(md_name): Path<String>,
    Json(req): Json<crate::routes::schemas::MdFileContent>,
) -> Result<Json<serde_json::Value>, AgentRouteError> {
    state
        .file_manager
        .write_working_md(&md_name, &req.content)
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "written": true })))
}

/// GET /api/agent/memory - List memory files.
pub async fn list_memory_files(
    State(state): State<AgentState>,
) -> Result<Json<Vec<crate::routes::schemas::MdFileInfo>>, AgentRouteError> {
    let files = state
        .file_manager
        .list_memory_mds()
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    let file_infos: Vec<crate::routes::schemas::MdFileInfo> = files
        .into_iter()
        .map(|f| crate::routes::schemas::MdFileInfo {
            filename: f.filename,
            path: f.path.display().to_string(),
            size: f.size,
            created_time: f.created_time,
            modified_time: f.modified_time,
        })
        .collect();

    Ok(Json(file_infos))
}

/// GET /api/agent/memory/:name - Read a memory file.
pub async fn read_memory_file(
    State(state): State<AgentState>,
    Path(md_name): Path<String>,
) -> Result<Json<crate::routes::schemas::MdFileContent>, AgentRouteError> {
    let content = state
        .file_manager
        .read_memory_md(&md_name)
        .map_err(|e| match e {
            copaw_agents::AgentFileManagerError::FileNotFound(_) => {
                AgentRouteError::NotFound(format!("Memory file not found: {}", md_name))
            }
            _ => AgentRouteError::Internal(e.to_string()),
        })?;

    Ok(Json(crate::routes::schemas::MdFileContent { content }))
}

/// PUT /api/agent/memory/:name - Write a memory file.
pub async fn write_memory_file(
    State(state): State<AgentState>,
    Path(md_name): Path<String>,
    Json(req): Json<crate::routes::schemas::MdFileContent>,
) -> Result<Json<serde_json::Value>, AgentRouteError> {
    state
        .file_manager
        .write_memory_md(&md_name, &req.content)
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "written": true })))
}

/// GET /api/agent/running-config - Get agent running config.
pub async fn get_running_config() -> Result<Json<copaw_config::AgentsRunningConfig>, AgentRouteError> {
    let config = load_config(None)
        .await
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    Ok(Json(config.agents.running))
}

/// PUT /api/agent/running-config - Update agent running config.
pub async fn update_running_config(
    Json(running_config): Json<copaw_config::AgentsRunningConfig>,
) -> Result<Json<copaw_config::AgentsRunningConfig>, AgentRouteError> {
    let mut config = load_config(None)
        .await
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    config.agents.running = running_config.clone();

    save_config(&config, None)
        .await
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    Ok(Json(running_config))
}

/// Create the agent router.
pub fn create_agent_router() -> axum::Router<AgentState> {
    use axum::routing::*;

    axum::Router::new()
        .route("/files", get(list_working_files))
        .route("/files/:md_name", get(read_working_file).put(write_working_file))
        .route("/memory", get(list_memory_files))
        .route("/memory/:md_name", get(read_memory_file).put(write_memory_file))
        .route("/running-config", get(get_running_config).put(update_running_config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use copaw_agents::AgentFileManager;

    struct TestState {
        state: AgentState,
        _temp_dir: TempDir,
    }

    async fn create_test_state() -> TestState {
        let temp_dir = TempDir::new().unwrap();
        let file_manager = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();
        TestState {
            state: AgentState {
                file_manager: Arc::new(file_manager),
            },
            _temp_dir: temp_dir,
        }
    }

    #[tokio::test]
    async fn test_list_working_files_empty() {
        let test_state = create_test_state().await;
        let result = list_working_files(State(test_state.state)).await;
        assert!(result.is_ok());
        let files = result.unwrap().0;
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_write_and_read_working_file() {
        let test_state = create_test_state().await;

        let req = crate::routes::schemas::MdFileContent {
            content: "# Test Content".to_string(),
        };

        let write_result = write_working_file(State(test_state.state.clone()), Path("test".to_string()), Json(req.clone())).await;
        assert!(write_result.is_ok());

        let read_result = read_working_file(State(test_state.state), Path("test".to_string())).await;
        assert!(read_result.is_ok());
        let content = read_result.unwrap().0;
        assert_eq!(content.content, "# Test Content");
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let test_state = create_test_state().await;
        let result = read_working_file(State(test_state.state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_memory_files_empty() {
        let test_state = create_test_state().await;
        let result = list_memory_files(State(test_state.state)).await;
        assert!(result.is_ok());
        let files = result.unwrap().0;
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_write_and_read_memory_file() {
        let test_state = create_test_state().await;

        let req = crate::routes::schemas::MdFileContent {
            content: "# Memory Content".to_string(),
        };

        let write_result = write_memory_file(State(test_state.state.clone()), Path("mem_test".to_string()), Json(req.clone())).await;
        assert!(write_result.is_ok());

        let read_result = read_memory_file(State(test_state.state), Path("mem_test".to_string())).await;
        assert!(read_result.is_ok());
        let content = read_result.unwrap().0;
        assert_eq!(content.content, "# Memory Content");
    }
}
