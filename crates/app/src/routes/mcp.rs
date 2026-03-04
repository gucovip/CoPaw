// -*- coding: utf-8 -*-
// API routes for MCP (Model Context Protocol) client management

use crate::mcp::{McpError, McpManager, McpTestResult};
use crate::routes::schemas::{McpClientCreateRequest, McpClientUpdateRequest};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// State for MCP routes
#[derive(Clone)]
pub struct McpState {
    config_path: Option<PathBuf>,
}

impl McpState {
    pub fn new(config_path: Option<PathBuf>) -> Self {
        Self { config_path }
    }

    pub fn get_config_path(&self) -> PathBuf {
        self.config_path.clone().unwrap_or_else(copaw_config::get_config_path)
    }

    pub fn manager(&self) -> McpManager {
        McpManager::new(self.config_path.clone())
    }
}

/// Trait for accessing MCP state
pub trait HasMcpState {
    fn mcp_state(&self) -> &McpState;
}

impl HasMcpState for McpState {
    fn mcp_state(&self) -> &McpState {
        self
    }
}

/// Error response
#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

/// MCP API error
#[derive(Debug)]
pub enum McpRouteError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for McpRouteError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            McpRouteError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            McpRouteError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            McpRouteError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        let body = Json(ErrorResponse { error: message });
        (status, body).into_response()
    }
}

/// GET /api/mcp - List all MCP clients
pub async fn list_mcp_clients<S>(State(state): State<S>) -> Result<Json<Vec<Value>>, McpRouteError>
where
    S: HasMcpState + Clone + Send + Sync + 'static,
{
    let manager = state.mcp_state().manager();
    let clients = manager
        .list_clients()
        .await
        .map_err(|e| McpRouteError::Internal(e.to_string()))?;

    let json_clients: Vec<Value> = clients
        .into_iter()
        .map(|c| serde_json::to_value(c).unwrap())
        .collect();

    Ok(Json(json_clients))
}

/// GET /api/mcp/{client_key} - Get MCP client details
pub async fn get_mcp_client<S>(
    State(state): State<S>,
    Path(client_key): Path<String>,
) -> Result<Json<Value>, McpRouteError>
where
    S: HasMcpState + Clone + Send + Sync + 'static,
{
    let manager = state.mcp_state().manager();
    let client = manager
        .get_client(&client_key)
        .await
        .map_err(|e| match e {
            McpError::NotFound(_) => McpRouteError::NotFound(e.to_string()),
            _ => McpRouteError::Internal(e.to_string()),
        })?;

    Ok(Json(serde_json::to_value(client).unwrap()))
}

/// POST /api/mcp - Create a new MCP client
pub async fn create_mcp_client<S>(
    State(state): State<S>,
    Json(req): Json<McpClientCreateRequest>,
) -> Result<Json<Value>, McpRouteError>
where
    S: HasMcpState + Clone + Send + Sync + 'static,
{
    let manager = state.mcp_state().manager();

    let client_config = copaw_config::MCPClientConfig {
        name: req.name,
        description: req.description,
        enabled: req.enabled,
        transport: copaw_config::TransportType::normalize(&req.transport),
        url: req.url,
        headers: req.headers,
        command: req.command,
        args: req.args,
        env: req.env,
        cwd: req.cwd,
    };

    let result = manager
        .create_client(&req.client_key, client_config)
        .await
        .map_err(|e| match e {
            McpError::AlreadyExists(_) => McpRouteError::BadRequest(e.to_string()),
            _ => McpRouteError::Internal(e.to_string()),
        })?;

    Ok(Json(serde_json::to_value(result).unwrap()))
}

/// PUT /api/mcp/{client_key} - Update an MCP client
pub async fn update_mcp_client<S>(
    State(state): State<S>,
    Path(client_key): Path<String>,
    Json(req): Json<McpClientUpdateRequest>,
) -> Result<Json<Value>, McpRouteError>
where
    S: HasMcpState + Clone + Send + Sync + 'static,
{
    let manager = state.mcp_state().manager();

    // Build updates map
    let mut updates = HashMap::new();
    if let Some(name) = req.name {
        updates.insert("name".to_string(), serde_json::to_value(name).unwrap());
    }
    if let Some(description) = req.description {
        updates.insert("description".to_string(), serde_json::to_value(description).unwrap());
    }
    if let Some(enabled) = req.enabled {
        updates.insert("enabled".to_string(), serde_json::to_value(enabled).unwrap());
    }
    if let Some(url) = req.url {
        updates.insert("url".to_string(), serde_json::to_value(url).unwrap());
    }
    if let Some(command) = req.command {
        updates.insert("command".to_string(), serde_json::to_value(command).unwrap());
    }
    if let Some(cwd) = req.cwd {
        updates.insert("cwd".to_string(), serde_json::to_value(cwd).unwrap());
    }
    if let Some(args) = req.args {
        updates.insert("args".to_string(), serde_json::to_value(args).unwrap());
    }
    if let Some(env) = req.env {
        updates.insert("env".to_string(), serde_json::to_value(env).unwrap());
    }
    if let Some(headers) = req.headers {
        updates.insert("headers".to_string(), serde_json::to_value(headers).unwrap());
    }

    let result = manager
        .update_client(&client_key, updates)
        .await
        .map_err(|e| match e {
            McpError::NotFound(_) => McpRouteError::NotFound(e.to_string()),
            _ => McpRouteError::Internal(e.to_string()),
        })?;

    Ok(Json(serde_json::to_value(result).unwrap()))
}

/// DELETE /api/mcp/{client_key} - Delete an MCP client
pub async fn delete_mcp_client<S>(
    State(state): State<S>,
    Path(client_key): Path<String>,
) -> Result<Json<Value>, McpRouteError>
where
    S: HasMcpState + Clone + Send + Sync + 'static,
{
    let manager = state.mcp_state().manager();

    manager
        .delete_client(&client_key)
        .await
        .map_err(|e| match e {
            McpError::NotFound(_) => McpRouteError::NotFound(e.to_string()),
            _ => McpRouteError::Internal(e.to_string()),
        })?;

    Ok(Json(serde_json::json!({
        "message": format!("MCP client '{}' deleted successfully", client_key)
    })))
}

/// POST /api/mcp/{client_key}/test - Test MCP connection
pub async fn test_mcp_client<S>(
    State(state): State<S>,
    Path(client_key): Path<String>,
) -> Result<Json<McpTestResult>, McpRouteError>
where
    S: HasMcpState + Clone + Send + Sync + 'static,
{
    let manager = state.mcp_state().manager();

    let result = manager
        .test_client(&client_key)
        .await
        .map_err(|e| match e {
            McpError::NotFound(_) => McpRouteError::NotFound(e.to_string()),
            _ => McpRouteError::Internal(e.to_string()),
        })?;

    Ok(Json(result))
}

/// PATCH /api/mcp/{client_key}/toggle - Toggle MCP client enabled status
pub async fn toggle_mcp_client<S>(
    State(state): State<S>,
    Path(client_key): Path<String>,
) -> Result<Json<Value>, McpRouteError>
where
    S: HasMcpState + Clone + Send + Sync + 'static,
{
    let manager = state.mcp_state().manager();

    let result = manager
        .toggle_client(&client_key)
        .await
        .map_err(|e| match e {
            McpError::NotFound(_) => McpRouteError::NotFound(e.to_string()),
            _ => McpRouteError::Internal(e.to_string()),
        })?;

    Ok(Json(serde_json::to_value(result).unwrap()))
}

/// Create the MCP router (generic over any state implementing HasMcpState)
pub fn create_mcp_router<S>() -> axum::Router<S>
where
    S: HasMcpState + Clone + Send + Sync + 'static,
{
    use axum::routing::*;

    axum::Router::new()
        .route("/", get(list_mcp_clients::<S>).post(create_mcp_client::<S>))
        .route("/:client_key", get(get_mcp_client::<S>).put(update_mcp_client::<S>).delete(delete_mcp_client::<S>))
        .route("/:client_key/test", post(test_mcp_client::<S>))
        .route("/:client_key/toggle", axum::routing::patch(toggle_mcp_client::<S>))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    async fn create_test_mcp_state() -> McpState {
        let config = copaw_config::CoPawConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        let _ = temp_file.into_temp_path();

        copaw_config::save_config(&config, Some(&path)).await.unwrap();
        McpState::new(Some(path))
    }

    #[tokio::test]
    async fn test_list_mcp_clients() {
        let state = create_test_mcp_state().await;

        let result = list_mcp_clients(State(state)).await;
        assert!(result.is_ok());

        let clients = result.unwrap().0;
        assert!(!clients.is_empty()); // Default has tavily_search
    }

    #[tokio::test]
    async fn test_get_mcp_client() {
        let state = create_test_mcp_state().await;

        let result = get_mcp_client(State(state), Path("tavily_search".to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_mcp_client_not_found() {
        let state = create_test_mcp_state().await;

        let result = get_mcp_client(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mcp_client_create_request() {
        let json = r#"{
            "client_key": "test_client",
            "name": "Test MCP",
            "description": "A test client",
            "command": "echo",
            "args": ["test"]
        }"#;
        let req: McpClientCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.client_key, "test_client");
        assert_eq!(req.name, "Test MCP");
        assert_eq!(req.command, "echo");
    }

    #[tokio::test]
    async fn test_mcp_client_update_request() {
        let json = r#"{"name": "Updated Name"}"#;
        let req: McpClientUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("Updated Name".to_string()));
        assert!(req.description.is_none());
    }
}
