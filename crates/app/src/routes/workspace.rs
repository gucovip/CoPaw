// -*- coding: utf-8 -*-
// API routes for workspace export/import.

use axum::{
    body::Body,
    extract::Multipart,
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
};
use copaw_config::get_working_dir;
use serde::Serialize;
use tokio::fs;

/// Error types for workspace API.
#[derive(Debug)]
pub enum WorkspaceError {
    NotFound(String),
    Internal(String),
}

impl IntoResponse for WorkspaceError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            WorkspaceError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            WorkspaceError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({ "detail": message }));
        (status, body).into_response()
    }
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceError::NotFound(msg) => write!(f, "Not found: {}", msg),
            WorkspaceError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for WorkspaceError {}

/// GET /api/workspace/download - Export workspace as zip
pub async fn download_workspace() -> Result<Response, WorkspaceError> {
    let working_dir = get_working_dir();

    if !working_dir.is_dir() {
        return Err(WorkspaceError::NotFound(format!(
            "Working directory does not exist: {:?}",
            working_dir
        )));
    }

    // For now, return a simple JSON response with workspace info
    // TODO: Implement actual zip export
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        Body::from(
            serde_json::json!({
                "message": "Workspace export not yet implemented",
                "working_dir": working_dir.to_string_lossy()
            })
            .to_string(),
        ),
    )
        .into_response())
}

/// Upload success response.
#[derive(Debug, Clone, Serialize)]
pub struct UploadSuccessResponse {
    pub success: bool,
}

/// POST /api/workspace/upload - Import and merge workspace from zip
pub async fn upload_workspace(
    _multipart: Multipart,
) -> Result<Json<UploadSuccessResponse>, WorkspaceError> {
    let working_dir = get_working_dir();

    if !working_dir.is_dir() {
        fs::create_dir_all(&working_dir).await.map_err(|e| {
            WorkspaceError::Internal(format!("Failed to create working directory: {}", e))
        })?;
    }

    // For now, return success
    // TODO: Implement actual zip import
    Ok(Json(UploadSuccessResponse { success: true }))
}

/// Create the workspace router (generic to accept any state).
pub fn create_workspace_router<S>() -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    use axum::routing::*;

    axum::Router::new()
        .route("/download", get(download_workspace))
        .route("/upload", post(upload_workspace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_error_display() {
        let error = WorkspaceError::NotFound("workspace not found".to_string());
        assert_eq!(format!("{}", error), "Not found: workspace not found");
    }

    #[tokio::test]
    async fn test_upload_success_response() {
        let response = UploadSuccessResponse { success: true };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
