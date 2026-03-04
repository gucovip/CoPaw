// -*- coding: utf-8 -*-
// API routes for environment variable management

use crate::envs::{EnvError, EnvStore};
use crate::routes::schemas::EnvVar;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// State for environment variable routes
#[derive(Clone)]
pub struct EnvsState {
    envs_path: Option<PathBuf>,
}

impl EnvsState {
    pub fn new(envs_path: Option<PathBuf>) -> Self {
        Self { envs_path }
    }

    pub fn store(&self) -> EnvStore {
        EnvStore::new(self.envs_path.clone())
    }
}

/// Trait for accessing envs state
pub trait HasEnvsState {
    fn envs_state(&self) -> &EnvsState;
}

impl HasEnvsState for EnvsState {
    fn envs_state(&self) -> &EnvsState {
        self
    }
}

/// Error response
#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

/// Environment variable API error
#[derive(Debug)]
pub enum EnvsError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for EnvsError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            EnvsError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            EnvsError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            EnvsError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        let body = Json(ErrorResponse { error: message });
        (status, body).into_response()
    }
}

/// GET /api/envs - List all environment variables (masked)
pub async fn list_envs<S>(State(state): State<S>) -> Result<Json<Vec<EnvVar>>, EnvsError>
where
    S: HasEnvsState + Clone + Send + Sync + 'static,
{
    let store = state.envs_state().store();
    let masked = store
        .list_masked()
        .await
        .map_err(|e| EnvsError::Internal(e.to_string()))?;

    let env_vars: Vec<EnvVar> = masked
        .into_iter()
        .map(|(k, v)| EnvVar { key: k, value: v })
        .collect();

    Ok(Json(env_vars))
}

/// POST /api/envs - Set an environment variable
pub async fn set_env_var<S>(
    State(state): State<S>,
    Json(body): Json<Value>,
) -> Result<Json<Vec<EnvVar>>, EnvsError>
where
    S: HasEnvsState + Clone + Send + Sync + 'static,
{
    let key = body
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| EnvsError::BadRequest("Missing 'key' field".to_string()))?;

    let value = body
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| EnvsError::BadRequest("Missing 'value' field".to_string()))?;

    let store = state.envs_state().store();
    let envs = store
        .set(key, value)
        .await
        .map_err(|e| match e {
            EnvError::InvalidKey(_) => EnvsError::BadRequest(e.to_string()),
            _ => EnvsError::Internal(e.to_string()),
        })?;

    let env_vars: Vec<EnvVar> = envs
        .into_iter()
        .map(|(k, v)| EnvVar {
            key: k,
            value: crate::envs::mask_env_value(&v),
        })
        .collect();

    Ok(Json(env_vars))
}

/// PUT /api/envs - Batch save environment variables
pub async fn batch_save_envs<S>(
    State(state): State<S>,
    Json(body): Json<Value>,
) -> Result<Json<Vec<EnvVar>>, EnvsError>
where
    S: HasEnvsState + Clone + Send + Sync + 'static,
{
    let envs_map: HashMap<String, String> = serde_json::from_value(body)
        .map_err(|e| EnvsError::BadRequest(format!("Invalid JSON object: {}", e)))?;

    let store = state.envs_state().store();
    let envs = store
        .batch_save(envs_map)
        .await
        .map_err(|e| match e {
            EnvError::InvalidKey(_) => EnvsError::BadRequest(e.to_string()),
            _ => EnvsError::Internal(e.to_string()),
        })?;

    let env_vars: Vec<EnvVar> = envs
        .into_iter()
        .map(|(k, v)| EnvVar {
            key: k,
            value: crate::envs::mask_env_value(&v),
        })
        .collect();

    Ok(Json(env_vars))
}

/// DELETE /api/envs/{key} - Delete an environment variable
pub async fn delete_env<S>(
    State(state): State<S>,
    Path(key): Path<String>,
) -> Result<Json<Vec<EnvVar>>, EnvsError>
where
    S: HasEnvsState + Clone + Send + Sync + 'static,
{
    let store = state.envs_state().store();
    let envs = store
        .delete(&key)
        .await
        .map_err(|e| match e {
            EnvError::NotFound(_) => EnvsError::NotFound(format!("Env var '{}' not found", key)),
            _ => EnvsError::Internal(e.to_string()),
        })?;

    let env_vars: Vec<EnvVar> = envs
        .into_iter()
        .map(|(k, v)| EnvVar {
            key: k,
            value: crate::envs::mask_env_value(&v),
        })
        .collect();

    Ok(Json(env_vars))
}

/// Create the environment variables router (generic over any state implementing HasEnvsState)
pub fn create_envs_router<S>() -> axum::Router<S>
where
    S: HasEnvsState + Clone + Send + Sync + 'static,
{
    use axum::routing::*;

    axum::Router::new()
        .route("/", get(list_envs::<S>).post(set_env_var::<S>).put(batch_save_envs::<S>))
        .route("/:key", axum::routing::delete(delete_env::<S>))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_envs_state() -> EnvsState {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("envs.json");
        EnvsState::new(Some(path))
    }

    #[tokio::test]
    async fn test_list_envs_empty() {
        let state = create_test_envs_state().await;

        let result = list_envs(State(state)).await;
        assert!(result.is_ok());

        let envs = result.unwrap().0;
        assert!(envs.is_empty());
    }

    #[tokio::test]
    async fn test_set_env_var() {
        let state = create_test_envs_state().await;

        let body = serde_json::json!({
            "key": "TEST_KEY",
            "value": "test_value"
        });

        let result = set_env_var(State(state), Json(body)).await;
        assert!(result.is_ok());

        let envs = result.unwrap().0;
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].key, "TEST_KEY");
    }

    #[tokio::test]
    async fn test_delete_env_var() {
        let state = create_test_envs_state().await;

        // First set a value
        let body = serde_json::json!({
            "key": "TO_DELETE",
            "value": "value"
        });
        let _ = set_env_var(State(state.clone()), Json(body.clone())).await;

        // Then delete it
        let result = delete_env(State(state), Path("TO_DELETE".to_string())).await;
        assert!(result.is_ok());

        let envs = result.unwrap().0;
        assert!(!envs.iter().any(|e| e.key == "TO_DELETE"));
    }

    #[tokio::test]
    async fn test_delete_env_var_not_found() {
        let state = create_test_envs_state().await;

        let result = delete_env(State(state), Path("NONEXISTENT".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_batch_save_envs() {
        let state = create_test_envs_state().await;

        let body = serde_json::json!({
            "KEY1": "value1",
            "KEY2": "value2"
        });

        let result = batch_save_envs(State(state), Json(body)).await;
        assert!(result.is_ok());

        let envs = result.unwrap().0;
        assert_eq!(envs.len(), 2);
    }
}
