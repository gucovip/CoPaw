// -*- coding: utf-8 -*-
// -*- mode: rust -*-
//! Cron manager for scheduling and executing jobs.
//!
//! This module provides the CronManager which handles job scheduling,
//! execution, pause/resume, matching the Python implementation in
//! src/copaw/app/crons/manager.py.

use crate::crons::job_repo::{JobRepoError, JobRepository, JobRepoResult};
use crate::crons::models::{CronJobSpec, CronJobState, JobStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;
use tracing::{debug, error, info, warn};

/// Error type for cron manager operations
#[derive(Debug, thiserror::Error)]
pub enum CronManagerError {
    #[error("Job not found: {0}")]
    NotFound(String),

    #[error("Scheduler error: {0}")]
    Scheduler(String),

    #[error("Repository error: {0}")]
    Repo(String),

    #[error("Invalid cron expression: {0}")]
    InvalidCron(String),

    #[error("Job execution error: {0}")]
    Execution(String),
}

/// Result type for cron manager operations
pub type CronManagerResult<T> = Result<T, CronManagerError>;

/// Runtime information for a job
#[derive(Debug)]
struct JobRuntime {
    /// Semaphore for limiting concurrent executions
    sem: Arc<tokio::sync::Semaphore>,
    /// Whether the job is paused
    paused: Arc<RwLock<bool>>,
}

/// Callback trait for executing cron jobs
///
/// This allows the manager to delegate actual job execution to the caller.
#[async_trait::async_trait]
pub trait JobExecutor: Send + Sync {
    /// Execute a cron job
    async fn execute_job(&self, job: &CronJobSpec) -> Result<(), String>;
}

/// Simplified UUID storage for job tracking
#[derive(Debug, Clone, Copy)]
struct JobUuid(Uuid);

/// Cron manager for scheduling and executing jobs
///
/// The manager maintains:
/// - A job repository for persistence
/// - A tokio-cron-scheduler for scheduling
/// - In-memory state tracking for next_run_at, last_run_at, etc.
/// - Per-job semaphores for concurrency control
pub struct CronManager {
    /// Job repository for persistence
    repo: Arc<JobRepository>,

    /// Job scheduler
    scheduler: Arc<RwLock<JobScheduler>>,

    /// In-memory job states (next_run_at, last_run_at, etc.)
    states: Arc<RwLock<HashMap<String, CronJobState>>>,

    /// Per-job runtime info (semaphores, etc.)
    runtimes: Arc<RwLock<HashMap<String, JobRuntime>>>,

    /// Job UUIDs by job ID
    job_uuids: Arc<RwLock<HashMap<String, JobUuid>>>,

    /// Job executor for running jobs
    executor: Arc<dyn JobExecutor>,

    /// Whether the manager has been started
    started: Arc<RwLock<bool>>,
}

impl CronManager {
    /// Create a new cron manager
    ///
    /// # Arguments
    ///
    /// * `repo` - Job repository for persistence
    /// * `executor` - Job executor for running jobs
    pub fn new(repo: JobRepository, executor: Arc<dyn JobExecutor>) -> Self {
        Self {
            repo: Arc::new(repo),
            scheduler: Arc::new(RwLock::new(
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        JobScheduler::new().await
                    })
                })
                .map_err(|e| {
                    error!("Failed to create scheduler: {}", e);
                    e
                })
                .unwrap(),
            )),
            states: Arc::new(RwLock::new(HashMap::new())),
            runtimes: Arc::new(RwLock::new(HashMap::new())),
            job_uuids: Arc::new(RwLock::new(HashMap::new())),
            executor,
            started: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the cron manager
    ///
    /// This loads all jobs from the repository and schedules them.
    pub async fn start(&self) -> CronManagerResult<()> {
        let mut started = self.started.write().await;
        if *started {
            return Ok(());
        }

        info!("Starting cron manager");

        // Load jobs from repository
        let jobs_file = self
            .repo
            .load()
            .await
            .map_err(|e| CronManagerError::Repo(e.to_string()))?;

        info!("Loaded {} jobs from repository", jobs_file.jobs.len());

        // Start the scheduler
        {
            let mut scheduler = self.scheduler.write().await;
            scheduler
                .start()
                .await
                .map_err(|e| CronManagerError::Scheduler(e.to_string()))?;
        }

        // Register each job
        for job in &jobs_file.jobs {
            if let Err(e) = self.register_or_update_job(job).await {
                warn!("Failed to register job {}: {}", job.id, e);
            }
        }

        *started = true;
        info!("Cron manager started with {} jobs", jobs_file.jobs.len());

        Ok(())
    }

    /// Stop the cron manager
    ///
    /// This stops the scheduler and clears all scheduled jobs.
    pub async fn stop(&self) -> CronManagerResult<()> {
        let mut started = self.started.write().await;
        if !*started {
            return Ok(());
        }

        info!("Stopping cron manager");

        {
            let mut scheduler = self.scheduler.write().await;
            scheduler
                .shutdown()
                .await
                .map_err(|e| CronManagerError::Scheduler(e.to_string()))?;
        }

        *started = false;
        info!("Cron manager stopped");

        Ok(())
    }

    /// List all jobs
    pub async fn list_jobs(&self) -> JobRepoResult<Vec<CronJobSpec>> {
        self.repo.list_jobs().await
    }

    /// Get a specific job by ID
    pub async fn get_job(&self, job_id: &str) -> JobRepoResult<Option<CronJobSpec>> {
        self.repo.get_job(job_id).await
    }

    /// Get the state of a job
    pub async fn get_state(&self, job_id: &str) -> CronJobState {
        let states = self.states.read().await;
        states.get(job_id).cloned().unwrap_or_default()
    }

    /// Create or update a job
    pub async fn create_or_replace_job(&self, spec: &CronJobSpec) -> CronManagerResult<()> {
        // Validate the job
        spec.validate()
            .map_err(|e| CronManagerError::InvalidCron(e))?;

        // Save to repository
        self.repo
            .upsert_job(spec)
            .await
            .map_err(|e| CronManagerError::Repo(e.to_string()))?;

        // Register in scheduler if started
        let started = *self.started.read().await;
        if started {
            self.register_or_update_job(spec).await?;
        }

        Ok(())
    }

    /// Delete a job
    pub async fn delete_job(&self, job_id: &str) -> CronManagerResult<bool> {
        // Remove from scheduler if present
        let started = *self.started.read().await;
        if started {
            if let Some(JobUuid(uuid)) = self.job_uuids.read().await.get(job_id).copied() {
                let scheduler = self.scheduler.read().await;
                let _ = scheduler.remove(&uuid).await;
            }
        }

        // Remove from state and runtime
        self.states.write().await.remove(job_id);
        self.runtimes.write().await.remove(job_id);
        self.job_uuids.write().await.remove(job_id);

        // Delete from repository
        self.repo
            .delete_job(job_id)
            .await
            .map_err(|e| CronManagerError::Repo(e.to_string()))
    }

    /// Pause a job
    pub async fn pause_job(&self, job_id: &str) -> CronManagerResult<()> {
        // Update job spec in repository
        let mut spec = self
            .repo
            .get_job(job_id)
            .await
            .map_err(|e| CronManagerError::Repo(e.to_string()))?
            .ok_or_else(|| CronManagerError::NotFound(job_id.to_string()))?;
        spec.enabled = false;
        self.repo
            .upsert_job(&spec)
            .await
            .map_err(|e| CronManagerError::Repo(e.to_string()))?;

        // Update runtime if exists (if manager is started)
        let mut runtimes = self.runtimes.write().await;
        if let Some(runtime) = runtimes.get_mut(job_id) {
            *runtime.paused.write().await = true;
        }

        info!("Paused job: {}", job_id);
        Ok(())
    }

    /// Resume a paused job
    pub async fn resume_job(&self, job_id: &str) -> CronManagerResult<()> {
        // Update job spec in repository
        let mut spec = self
            .repo
            .get_job(job_id)
            .await
            .map_err(|e| CronManagerError::Repo(e.to_string()))?
            .ok_or_else(|| CronManagerError::NotFound(job_id.to_string()))?;
        spec.enabled = true;
        self.repo
            .upsert_job(&spec)
            .await
            .map_err(|e| CronManagerError::Repo(e.to_string()))?;

        // Update runtime if exists (if manager is started)
        let mut runtimes = self.runtimes.write().await;
        if let Some(runtime) = runtimes.get_mut(job_id) {
            *runtime.paused.write().await = false;
        }

        info!("Resumed job: {}", job_id);
        Ok(())
    }

    /// Run a job immediately (fire-and-forget)
    pub async fn run_job(&self, job_id: &str) -> CronManagerResult<()> {
        let job = self
            .repo
            .get_job(job_id)
            .await
            .map_err(|e| CronManagerError::Repo(e.to_string()))?
            .ok_or_else(|| CronManagerError::NotFound(job_id.to_string()))?;

        info!(
            "Running job immediately: {} (type: {:?})",
            job_id, job.task_type
        );

        // Execute in background
        let executor = self.executor.clone();
        let job_clone = job.clone();
        let states = self.states.clone();
        let job_id = job_id.to_string();

        tokio::spawn(async move {
            // Update state to running
            {
                let mut states_guard = states.write().await;
                let state = states_guard.entry(job_id.clone()).or_default();
                state.last_status = Some(JobStatus::Running);
            }

            // Execute the job
            let result = executor.execute_job(&job_clone).await;

            // Update state based on result
            let mut states_guard = states.write().await;
            let state = states_guard.entry(job_id.clone()).or_default();

            match result {
                Ok(_) => {
                    state.last_status = Some(JobStatus::Success);
                    state.last_error = None;
                    info!("Job {} completed successfully", job_id);
                }
                Err(e) => {
                    state.last_status = Some(JobStatus::Error);
                    state.last_error = Some(e);
                    error!("Job {} failed: {}", job_id, state.last_error.as_ref().unwrap());
                }
            }

            state.last_run_at = Some(chrono::Utc::now());
        });

        Ok(())
    }

    /// Register or update a job in the scheduler
    async fn register_or_update_job(&self, spec: &CronJobSpec) -> CronManagerResult<()> {
        // Setup runtime (semaphore for concurrency control)
        let runtime = JobRuntime {
            sem: Arc::new(tokio::sync::Semaphore::new(spec.runtime.max_concurrency)),
            paused: Arc::new(RwLock::new(!spec.enabled)),
        };
        self.runtimes
            .write()
            .await
            .insert(spec.id.clone(), runtime);

        // Normalize cron expression
        let cron_expr = crate::crons::models::ScheduleSpec::normalize_cron(&spec.schedule.cron)
            .map_err(|e| CronManagerError::InvalidCron(e))?;

        // Create the scheduled job
        let job_id = spec.id.clone();
        let executor = self.executor.clone();
        let spec_clone = spec.clone();
        let states = self.states.clone();
        let runtimes = self.runtimes.clone();

        let job = Job::new_async(&cron_expr, move |_uuid, _l| {
            let job_id = job_id.clone();
            let executor = executor.clone();
            let spec_clone = spec_clone.clone();
            let states = states.clone();
            let runtimes = runtimes.clone();

            Box::pin(async move {
                debug!("Executing scheduled job: {}", job_id);

                // Check if paused
                let paused = {
                    let runtimes_guard = runtimes.read().await;
                    if let Some(runtime) = runtimes_guard.get(&job_id) {
                        *runtime.paused.read().await
                    } else {
                        false
                    }
                };

                if paused {
                    debug!("Job {} is paused, skipping execution", job_id);
                    return;
                }

                // Get runtime semaphore
                let sem = {
                    let runtimes_guard = runtimes.read().await;
                    runtimes_guard
                        .get(&job_id)
                        .map(|r| r.sem.clone())
                        .unwrap_or_else(|| {
                            // Fallback semaphore if runtime not found
                            Arc::new(tokio::sync::Semaphore::new(1))
                        })
                };

                // Acquire semaphore (respect max_concurrency)
                let _permit = sem.acquire().await.unwrap();

                // Update state to running
                {
                    let mut states_guard = states.write().await;
                    let state = states_guard.entry(job_id.clone()).or_default();
                    state.last_status = Some(JobStatus::Running);
                }

                // Execute the job
                let result = executor.execute_job(&spec_clone).await;

                // Update state based on result
                let mut states_guard = states.write().await;
                let state = states_guard.entry(job_id.clone()).or_default();

                match result {
                    Ok(_) => {
                        state.last_status = Some(JobStatus::Success);
                        state.last_error = None;
                        info!("Scheduled job {} completed successfully", job_id);
                    }
                    Err(e) => {
                        state.last_status = Some(JobStatus::Error);
                        state.last_error = Some(e);
                        error!(
                            "Scheduled job {} failed: {}",
                            job_id,
                            state.last_error.as_ref().unwrap()
                        );
                    }
                }

                state.last_run_at = Some(chrono::Utc::now());
            })
        })
        .map_err(|e| CronManagerError::Scheduler(e.to_string()))?;

        // Add job to scheduler
        let uuid = {
            let scheduler = self.scheduler.read().await;
            scheduler
                .add(job)
                .await
                .map_err(|e| CronManagerError::Scheduler(e.to_string()))?
        };

        // Store UUID for later operations
        self.job_uuids
            .write()
            .await
            .insert(spec.id.clone(), JobUuid(uuid));

        info!("Registered job: {} ({})", spec.id, spec.name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::sync::Arc;

    // Mock executor for testing
    struct MockExecutor;

    #[async_trait::async_trait]
    impl JobExecutor for MockExecutor {
        async fn execute_job(&self, _job: &CronJobSpec) -> Result<(), String> {
            debug!("Mock executor running job");
            Ok(())
        }
    }

    fn create_test_manager() -> (CronManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let jobs_path = temp_dir.path().join("jobs.json");
        let repo = JobRepository::new(&jobs_path);
        let executor = Arc::new(MockExecutor);
        let manager = CronManager::new(repo, executor);
        (manager, temp_dir)
    }

    fn create_test_job(id: &str, name: &str) -> CronJobSpec {
        serde_json::from_value(serde_json::json!({
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
    async fn test_manager_create() {
        let (manager, _temp) = create_test_manager();
        assert!(!*manager.started.read().await);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_create_or_replace_job() {
        let (manager, _temp) = create_test_manager();

        let job = create_test_job("job1", "Test Job");
        manager.create_or_replace_job(&job).await.unwrap();

        let jobs = manager.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "job1");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_get_job() {
        let (manager, _temp) = create_test_manager();

        let job = create_test_job("job1", "Test Job");
        manager.create_or_replace_job(&job).await.unwrap();

        let found = manager.get_job("job1").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test Job");

        let not_found = manager.get_job("job2").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_delete_job() {
        let (manager, _temp) = create_test_manager();

        let job = create_test_job("job1", "Test Job");
        manager.create_or_replace_job(&job).await.unwrap();

        let deleted = manager.delete_job("job1").await.unwrap();
        assert!(deleted);

        let jobs = manager.list_jobs().await.unwrap();
        assert!(jobs.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_get_state() {
        let (manager, _temp) = create_test_manager();

        let state = manager.get_state("job1").await;
        assert_eq!(state, CronJobState::default());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_run_job_immediately() {
        let (manager, _temp) = create_test_manager();

        let job = create_test_job("job1", "Test Job");
        manager.create_or_replace_job(&job).await.unwrap();

        // Run job immediately
        manager.run_job("job1").await.unwrap();

        // Give the background task time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check state
        let state = manager.get_state("job1").await;
        assert_eq!(state.last_status, Some(JobStatus::Success));
        assert!(state.last_run_at.is_some());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_pause_resume_job() {
        let (manager, _temp) = create_test_manager();

        let job = create_test_job("job1", "Test Job");
        manager.create_or_replace_job(&job).await.unwrap();

        // Pause the job
        manager.pause_job("job1").await.unwrap();

        // Resume the job
        manager.resume_job("job1").await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_invalid_job_spec() {
        let (manager, _temp) = create_test_manager();

        // Create invalid job (text type but empty text)
        let job: CronJobSpec = serde_json::from_value(serde_json::json!({
            "id": "job1",
            "name": "Invalid Job",
            "schedule": {
                "cron": "0 0 * * *"
            },
            "task_type": "text",
            "text": "",
            "dispatch": {
                "target": {
                    "user_id": "user123",
                    "session_id": "session456"
                }
            }
        }))
        .unwrap();

        let result = manager.create_or_replace_job(&job).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_cron_manager_error_display() {
        let err = CronManagerError::NotFound("job1".to_string());
        assert_eq!(err.to_string(), "Job not found: job1");

        let err = CronManagerError::InvalidCron("bad cron".to_string());
        assert_eq!(err.to_string(), "Invalid cron expression: bad cron");
    }
}
