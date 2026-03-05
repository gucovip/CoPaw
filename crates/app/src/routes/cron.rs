// -*- coding: utf-8 -*-
// -*- mode: rust -*-
//! API routes for cron job management.
//!
//! This module provides the HTTP API endpoints for managing cron jobs,
//! matching the Python implementation in src/copaw/app/crons/api.py.

use super::schemas::{CronJobStateResponse, JobPauseResponse, JobResumeResponse, JobRunResponse};
use crate::crons::{
    CronJobSpec, CronJobView, CronManager, CronManagerError, JobExecutor, JobRepository,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    Json as JsonExtractor,
};
use std::sync::Arc;
use uuid::Uuid;

/// Application state for cron routes.
#[derive(Clone)]
pub struct CronState {
    pub manager: Arc<CronManager>,
    pub repo: Arc<JobRepository>,
}

impl CronState {
    /// Create a new CronState with optional custom jobs path
    pub fn new(jobs_path: Option<std::path::PathBuf>) -> Self {
        let repo = Arc::new(JobRepository::new(
            jobs_path.unwrap_or_else(JobRepository::default_path),
        ));

        // Create a mock executor for now
        // TODO: Replace with proper executor integration
        use copaw_channels::ChannelManager;
        let channel_manager = Arc::new(ChannelManager::new());
        let executor: Arc<dyn JobExecutor> = Arc::new(MockCronExecutor {
            _channel_manager: channel_manager,
        });

        let manager = Arc::new(CronManager::new((*repo).clone(), executor));

        Self { manager, repo }
    }
}

/// Trait for types that have a cron state.
pub trait HasCronState {
    fn cron_state(&self) -> &CronState;
}

impl HasCronState for CronState {
    fn cron_state(&self) -> &CronState {
        self
    }
}

/// Mock executor for cron jobs
#[derive(Clone)]
struct MockCronExecutor {
    _channel_manager: Arc<copaw_channels::ChannelManager>,
}

#[async_trait::async_trait]
impl JobExecutor for MockCronExecutor {
    async fn execute_job(&self, job: &CronJobSpec) -> Result<(), String> {
        tracing::info!("Mock executor executing job: {} ({})", job.id, job.name);

        // For text-type jobs, just log the text
        if let Some(text) = &job.text {
            tracing::info!("Job text: {}", text);
        }

        // For agent-type jobs, log the request
        if let Some(request) = &job.request {
            tracing::info!("Job request input: {}", request.input);
        }

        Ok(())
    }
}

/// Error types for cron routes.
#[derive(Debug)]
pub enum CronApiError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for CronApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            CronApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            CronApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            CronApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({ "error": message }));
        (status, body).into_response()
    }
}

impl From<CronManagerError> for CronApiError {
    fn from(err: CronManagerError) -> Self {
        match err {
            CronManagerError::NotFound(msg) => CronApiError::NotFound(msg),
            CronManagerError::InvalidCron(msg) => CronApiError::BadRequest(msg),
            _ => CronApiError::Internal(err.to_string()),
        }
    }
}

/// GET /api/cron/jobs - List all jobs
pub async fn list_jobs<S>(State(state): State<S>) -> Result<Json<Vec<CronJobSpec>>, CronApiError>
where
    S: HasCronState + Clone + Send + Sync + 'static,
{
    let jobs = state
        .cron_state()
        .manager
        .list_jobs()
        .await
        .map_err(|e| CronApiError::Internal(e.to_string()))?;

    Ok(Json(jobs))
}

/// POST /api/cron/jobs - Create a new job
pub async fn create_job<S>(
    State(state): State<S>,
    JsonExtractor(mut spec): JsonExtractor<CronJobSpec>,
) -> Result<Json<CronJobSpec>, CronApiError>
where
    S: HasCronState + Clone + Send + Sync + 'static,
{
    // Server generates ID; ignore client-provided spec.id
    let job_id = Uuid::new_v4().to_string();
    spec.id = job_id.clone();

    state
        .cron_state()
        .manager
        .create_or_replace_job(&spec)
        .await?;

    Ok(Json(spec))
}

/// GET /api/cron/jobs/{id} - Get job details (spec + state)
pub async fn get_job<S>(
    State(state): State<S>,
    Path(job_id): Path<String>,
) -> Result<Json<CronJobView>, CronApiError>
where
    S: HasCronState + Clone + Send + Sync + 'static,
{
    let spec = state
        .cron_state()
        .repo
        .get_job(&job_id)
        .await
        .map_err(|e| CronApiError::Internal(e.to_string()))?
        .ok_or_else(|| CronApiError::NotFound(format!("Job not found: {}", job_id)))?;

    let state_data = state.cron_state().manager.get_state(&job_id).await;

    Ok(Json(CronJobView {
        spec,
        state: state_data,
    }))
}

/// PUT /api/cron/jobs/{id} - Update a job
pub async fn update_job<S>(
    State(state): State<S>,
    Path(job_id): Path<String>,
    JsonExtractor(spec): JsonExtractor<CronJobSpec>,
) -> Result<Json<CronJobSpec>, CronApiError>
where
    S: HasCronState + Clone + Send + Sync + 'static,
{
    if spec.id != job_id {
        return Err(CronApiError::BadRequest("job_id mismatch".to_string()));
    }

    state
        .cron_state()
        .manager
        .create_or_replace_job(&spec)
        .await?;

    Ok(Json(spec))
}

/// DELETE /api/cron/jobs/{id} - Delete a job
pub async fn delete_job<S>(
    State(state): State<S>,
    Path(job_id): Path<String>,
) -> Result<Json<serde_json::Value>, CronApiError>
where
    S: HasCronState + Clone + Send + Sync + 'static,
{
    let deleted = state
        .cron_state()
        .manager
        .delete_job(&job_id)
        .await
        .map_err(|e| CronApiError::Internal(e.to_string()))?;

    if !deleted {
        return Err(CronApiError::NotFound(format!("Job not found: {}", job_id)));
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// POST /api/cron/jobs/{id}/pause - Pause a job
pub async fn pause_job<S>(
    State(state): State<S>,
    Path(job_id): Path<String>,
) -> Result<Json<JobPauseResponse>, CronApiError>
where
    S: HasCronState + Clone + Send + Sync + 'static,
{
    state.cron_state().manager.pause_job(&job_id).await?;

    Ok(Json(JobPauseResponse { paused: true }))
}

/// POST /api/cron/jobs/{id}/resume - Resume a paused job
pub async fn resume_job<S>(
    State(state): State<S>,
    Path(job_id): Path<String>,
) -> Result<Json<JobResumeResponse>, CronApiError>
where
    S: HasCronState + Clone + Send + Sync + 'static,
{
    state.cron_state().manager.resume_job(&job_id).await?;

    Ok(Json(JobResumeResponse { resumed: true }))
}

/// POST /api/cron/jobs/{id}/run - Run a job immediately
pub async fn run_job<S>(
    State(state): State<S>,
    Path(job_id): Path<String>,
) -> Result<Json<JobRunResponse>, CronApiError>
where
    S: HasCronState + Clone + Send + Sync + 'static,
{
    state.cron_state().manager.run_job(&job_id).await?;

    Ok(Json(JobRunResponse { started: true }))
}

/// GET /api/cron/jobs/{id}/state - Get job state
pub async fn get_job_state<S>(
    State(state): State<S>,
    Path(job_id): Path<String>,
) -> Result<Json<CronJobStateResponse>, CronApiError>
where
    S: HasCronState + Clone + Send + Sync + 'static,
{
    // Verify job exists
    let _spec = state
        .cron_state()
        .repo
        .get_job(&job_id)
        .await
        .map_err(|e| CronApiError::Internal(e.to_string()))?
        .ok_or_else(|| CronApiError::NotFound(format!("Job not found: {}", job_id)))?;

    let state_data = state.cron_state().manager.get_state(&job_id).await;

    Ok(Json(CronJobStateResponse {
        next_run_at: state_data.next_run_at,
        last_run_at: state_data.last_run_at,
        last_status: state_data.last_status,
        last_error: state_data.last_error,
    }))
}

/// Create the cron router (generic over any state implementing HasCronState)
pub fn create_cron_router<S>() -> axum::Router<S>
where
    S: HasCronState + Clone + Send + Sync + 'static,
{
    use axum::routing::*;

    axum::Router::new()
        .route("/jobs", get(list_jobs::<S>).post(create_job::<S>))
        .route(
            "/jobs/:job_id",
            get(get_job::<S>)
                .put(update_job::<S>)
                .delete(delete_job::<S>),
        )
        .route("/jobs/:job_id/pause", post(pause_job::<S>))
        .route("/jobs/:job_id/resume", post(resume_job::<S>))
        .route("/jobs/:job_id/run", post(run_job::<S>))
        .route("/jobs/:job_id/state", get(get_job_state::<S>))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crons::{CronJobSpec, JobRepository};
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::TempDir;

    // Mock executor for testing
    struct MockExecutor;

    #[async_trait::async_trait]
    impl JobExecutor for MockExecutor {
        async fn execute_job(&self, _job: &CronJobSpec) -> Result<(), String> {
            Ok(())
        }
    }

    fn create_test_state() -> (CronState, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let jobs_path = temp_dir.path().join("jobs.json");
        let repo_inner = JobRepository::new(&jobs_path);
        let repo = Arc::new(repo_inner.clone());
        let manager = Arc::new(CronManager::new(repo_inner, Arc::new(MockExecutor)));
        let state = CronState { manager, repo };
        (state, temp_dir)
    }

    fn create_test_job(id: &str, name: &str) -> CronJobSpec {
        serde_json::from_value(json!({
            "id": id,
            "name": name,
            "schedule": {
                "cron": "0 0 * * *"
            },
            "task_type": "text",
            "text": "Hello from cron!",
            "dispatch": {
                "target": {
                    "user_id": "user123",
                    "session_id": "session456"
                }
            }
        }))
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_list_jobs_empty() {
        let (state, _temp) = create_test_state();

        let response = list_jobs(State(state)).await.unwrap().0;

        assert!(response.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_create_job() {
        let (state, _temp) = create_test_state();

        let job = create_test_job("test-id", "Test Job");

        let response = create_job(State(state), JsonExtractor(job))
            .await
            .unwrap()
            .0;

        // ID should be regenerated
        assert_ne!(response.id, "test-id");
        assert_eq!(response.name, "Test Job");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_get_job_not_found() {
        let (state, _temp) = create_test_state();

        let result = get_job(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CronApiError::NotFound(_) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_update_job_id_mismatch() {
        let (state, _temp) = create_test_state();

        let job = create_test_job("job1", "Test Job");

        let result = update_job(State(state), Path("job2".to_string()), JsonExtractor(job)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CronApiError::BadRequest(_) => {}
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_delete_job_not_found() {
        let (state, _temp) = create_test_state();

        let result = delete_job(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CronApiError::NotFound(_) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_get_job_state_not_found() {
        let (state, _temp) = create_test_state();

        let result = get_job_state(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CronApiError::NotFound(_) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_cron_api_error_into_response() {
        let error = CronApiError::NotFound("test".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let error = CronApiError::BadRequest("test".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let error = CronApiError::Internal("test".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_create_and_get_job() {
        let (state, _temp) = create_test_state();

        // Create job
        let job = create_test_job("test-id", "Test Job");
        let created = create_job(State(state.clone()), JsonExtractor(job))
            .await
            .unwrap()
            .0;

        let job_id = created.id.clone();

        // Get job
        let response = get_job(State(state), Path(job_id)).await.unwrap().0;
        assert_eq!(response.spec.name, "Test Job");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_create_and_delete_job() {
        let (state, _temp) = create_test_state();

        // Create job
        let job = create_test_job("test-id", "Test Job");
        let created = create_job(State(state.clone()), JsonExtractor(job))
            .await
            .unwrap()
            .0;

        let job_id = created.id.clone();

        // Delete job
        let response = delete_job(State(state), Path(job_id)).await.unwrap().0;
        assert_eq!(response["deleted"], true);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_pause_resume_job() {
        let (state, _temp) = create_test_state();

        // Start manager first
        state.manager.start().await.unwrap();

        // Create job
        let job = create_test_job("test-id", "Test Job");
        let created = create_job(State(state.clone()), JsonExtractor(job))
            .await
            .unwrap()
            .0;

        let job_id = created.id.clone();

        // Pause job
        let pause_response = pause_job(State(state.clone()), Path(job_id.clone()))
            .await
            .unwrap()
            .0;
        assert!(pause_response.paused);

        // Resume job
        let resume_response = resume_job(State(state.clone()), Path(job_id))
            .await
            .unwrap()
            .0;
        assert!(resume_response.resumed);

        state.manager.stop().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_run_job_immediately() {
        let (state, _temp) = create_test_state();

        // Create job
        let job = create_test_job("test-id", "Test Job");
        let created = create_job(State(state.clone()), JsonExtractor(job))
            .await
            .unwrap()
            .0;

        let job_id = created.id.clone();

        // Run job
        let response = run_job(State(state.clone()), Path(job_id)).await.unwrap().0;
        assert!(response.started);

        // Give background task time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check state
        let state_data = state.manager.get_state(&created.id).await;
        assert!(state_data.last_run_at.is_some());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_get_job_state() {
        let (state, _temp) = create_test_state();

        // Create job
        let job = create_test_job("test-id", "Test Job");
        let created = create_job(State(state.clone()), JsonExtractor(job))
            .await
            .unwrap()
            .0;

        let job_id = created.id.clone();

        // Get state
        let response = get_job_state(State(state), Path(job_id)).await.unwrap().0;

        assert!(response.next_run_at.is_none()); // Not scheduled until manager starts
        assert!(response.last_run_at.is_none());
        assert!(response.last_status.is_none());
        assert!(response.last_error.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_list_jobs_with_data() {
        let (state, _temp) = create_test_state();

        // Create two jobs
        let job1 = create_test_job("id1", "Job 1");
        let job2 = create_test_job("id2", "Job 2");

        let _ = create_job(State(state.clone()), JsonExtractor(job1))
            .await
            .unwrap();
        let _ = create_job(State(state.clone()), JsonExtractor(job2))
            .await
            .unwrap();

        // List jobs
        let response = list_jobs(State(state)).await.unwrap().0;
        assert_eq!(response.len(), 2);
    }
}
