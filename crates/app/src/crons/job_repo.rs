// -*- coding: utf-8 -*-
// -*- mode: rust -*-
//! Job repository for persisting cron jobs.
//!
//! This module provides JSON-based storage for cron job definitions,
//! matching the Python implementation in src/copaw/app/crons/repo/json_repo.py.

use super::models::{CronJobSpec, JobsFile};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Error type for job repository operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum JobRepoError {
    #[error("IO error: {0}")]
    Io(String),

    #[error("JSON parse error: {0}")]
    JsonParse(String),

    #[error("Job not found: {0}")]
    #[allow(dead_code)]
    NotFound(String),
}

/// Result type for job repository operations
pub type JobRepoResult<T> = Result<T, JobRepoError>;

/// Job repository for persisting cron jobs to JSON files
///
/// Jobs are stored in a single jobs.json file in the configured directory.
/// The repository uses atomic writes (write to temp file, then replace) for safety.
#[derive(Debug, Clone)]
pub struct JobRepository {
    /// Path to the jobs.json file
    path: PathBuf,
}

impl JobRepository {
    /// Create a new job repository with the given path
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the jobs.json file (will be created if it doesn't exist)
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Get the path to the jobs.json file
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the default jobs.json path
    ///
    /// Uses `~/.copaw/jobs/jobs.json` by default
    pub fn default_path() -> PathBuf {
        // Get the home directory
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());

        PathBuf::from(home_dir)
            .join(".copaw")
            .join("jobs")
            .join("jobs.json")
    }

    /// Load jobs from the repository
    ///
    /// Returns an empty JobsFile if the file doesn't exist
    pub async fn load(&self) -> JobRepoResult<JobsFile> {
        // If file doesn't exist, return empty jobs file
        if !self.path.exists() {
            return Ok(JobsFile::default());
        }

        // Read the file
        let content = fs::read_to_string(&self.path)
            .await
            .map_err(|e| JobRepoError::Io(format!("Failed to read jobs file: {}", e)))?;

        // Parse JSON
        serde_json::from_str(&content)
            .map_err(|e| JobRepoError::JsonParse(format!("Failed to parse jobs JSON: {}", e)))
    }

    /// Save jobs to the repository
    ///
    /// Uses atomic write (temp file + replace) for safety
    pub async fn save(&self, jobs_file: &JobsFile) -> JobRepoResult<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| JobRepoError::Io(format!("Failed to create jobs directory: {}", e)))?;
        }

        // Serialize to JSON
        let json = serde_json::to_string_pretty(jobs_file)
            .map_err(|e| JobRepoError::JsonParse(format!("Failed to serialize jobs: {}", e)))?;

        // Write to temp file first
        let temp_path = self.path.with_extension("json.tmp");
        let mut file = fs::File::create(&temp_path)
            .await
            .map_err(|e| JobRepoError::Io(format!("Failed to create temp file: {}", e)))?;

        file.write_all(json.as_bytes())
            .await
            .map_err(|e| JobRepoError::Io(format!("Failed to write to temp file: {}", e)))?;

        file.flush()
            .await
            .map_err(|e| JobRepoError::Io(format!("Failed to flush temp file: {}", e)))?;
        drop(file);

        // Replace original file with temp file
        fs::rename(&temp_path, &self.path)
            .await
            .map_err(|e| JobRepoError::Io(format!("Failed to rename temp file: {}", e)))?;

        Ok(())
    }

    /// List all jobs
    pub async fn list_jobs(&self) -> JobRepoResult<Vec<CronJobSpec>> {
        let jobs_file = self.load().await?;
        Ok(jobs_file.jobs)
    }

    /// Get a specific job by ID
    pub async fn get_job(&self, job_id: &str) -> JobRepoResult<Option<CronJobSpec>> {
        let jobs_file = self.load().await?;
        Ok(jobs_file.jobs.into_iter().find(|j| j.id == job_id))
    }

    /// Create or update a job
    pub async fn upsert_job(&self, spec: &CronJobSpec) -> JobRepoResult<()> {
        let mut jobs_file = self.load().await?;

        // Remove existing job with same ID if present
        jobs_file.jobs.retain(|j| j.id != spec.id);

        // Add the new/updated job
        jobs_file.jobs.push(spec.clone());

        self.save(&jobs_file).await
    }

    /// Delete a job by ID
    ///
    /// Returns true if the job was found and deleted, false otherwise
    pub async fn delete_job(&self, job_id: &str) -> JobRepoResult<bool> {
        let mut jobs_file = self.load().await?;

        let original_len = jobs_file.jobs.len();
        jobs_file.jobs.retain(|j| j.id != job_id);

        let found = jobs_file.jobs.len() < original_len;

        if found {
            self.save(&jobs_file).await?;
        }

        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn create_test_repo() -> (JobRepository, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let jobs_path = temp_dir.path().join("jobs.json");
        let repo = JobRepository::new(&jobs_path);
        (repo, temp_dir)
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

    #[tokio::test]
    async fn test_load_empty() {
        let (repo, _temp) = create_test_repo();
        let jobs_file = repo.load().await.unwrap();
        assert_eq!(jobs_file.version, 1);
        assert!(jobs_file.jobs.is_empty());
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let (repo, _temp) = create_test_repo();

        let job = create_test_job("job1", "Test Job");
        let jobs_file = JobsFile {
            version: 1,
            jobs: vec![job.clone()],
        };

        repo.save(&jobs_file).await.unwrap();

        let loaded = repo.load().await.unwrap();
        assert_eq!(loaded.jobs.len(), 1);
        assert_eq!(loaded.jobs[0].id, "job1");
        assert_eq!(loaded.jobs[0].name, "Test Job");
    }

    #[tokio::test]
    async fn test_list_jobs() {
        let (repo, _temp) = create_test_repo();

        let job1 = create_test_job("job1", "Job 1");
        let job2 = create_test_job("job2", "Job 2");

        let jobs_file = JobsFile {
            version: 1,
            jobs: vec![job1, job2],
        };

        repo.save(&jobs_file).await.unwrap();

        let jobs = repo.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 2);
    }

    #[tokio::test]
    async fn test_get_job() {
        let (repo, _temp) = create_test_repo();

        let job1 = create_test_job("job1", "Job 1");
        let job2 = create_test_job("job2", "Job 2");

        let jobs_file = JobsFile {
            version: 1,
            jobs: vec![job1.clone(), job2],
        };

        repo.save(&jobs_file).await.unwrap();

        let found = repo.get_job("job1").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "job1");

        let not_found = repo.get_job("job3").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_upsert_job_create() {
        let (repo, _temp) = create_test_repo();

        let job = create_test_job("job1", "New Job");
        repo.upsert_job(&job).await.unwrap();

        let jobs = repo.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "job1");
    }

    #[tokio::test]
    async fn test_upsert_job_update() {
        let (repo, _temp) = create_test_repo();

        let job1 = create_test_job("job1", "Original Job");
        repo.upsert_job(&job1).await.unwrap();

        let mut job1_updated = create_test_job("job1", "Updated Job");
        job1_updated.text = Some("Updated text".to_string());
        repo.upsert_job(&job1_updated).await.unwrap();

        let jobs = repo.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "Updated Job");
        assert_eq!(jobs[0].text.as_deref(), Some("Updated text"));
    }

    #[tokio::test]
    async fn test_delete_job() {
        let (repo, _temp) = create_test_repo();

        let job1 = create_test_job("job1", "Job 1");
        let job2 = create_test_job("job2", "Job 2");

        let jobs_file = JobsFile {
            version: 1,
            jobs: vec![job1, job2],
        };

        repo.save(&jobs_file).await.unwrap();

        // Delete existing job
        let deleted = repo.delete_job("job1").await.unwrap();
        assert!(deleted);

        let jobs = repo.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "job2");

        // Delete non-existing job
        let deleted = repo.delete_job("job3").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_atomic_write() {
        let (repo, _temp) = create_test_repo();

        // Write initial data
        let job1 = create_test_job("job1", "Job 1");
        let jobs_file = JobsFile {
            version: 1,
            jobs: vec![job1],
        };
        repo.save(&jobs_file).await.unwrap();

        // Write again (should replace atomically)
        let job2 = create_test_job("job2", "Job 2");
        let jobs_file = JobsFile {
            version: 1,
            jobs: vec![job2],
        };
        repo.save(&jobs_file).await.unwrap();

        let loaded = repo.load().await.unwrap();
        assert_eq!(loaded.jobs.len(), 1);
        assert_eq!(loaded.jobs[0].id, "job2");
    }

    #[test]
    fn test_default_path() {
        let path = JobRepository::default_path();
        assert!(path.ends_with(".copaw/jobs/jobs.json"));
    }

    #[tokio::test]
    async fn test_repo_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let jobs_path = temp_dir.path().join("nested").join("dir").join("jobs.json");
        let repo = JobRepository::new(&jobs_path);

        let job = create_test_job("job1", "Job 1");
        let jobs_file = JobsFile {
            version: 1,
            jobs: vec![job],
        };

        repo.save(&jobs_file).await.unwrap();

        // Verify the directory was created
        assert!(jobs_path.parent().unwrap().exists());
        assert!(jobs_path.exists());
    }

    #[tokio::test]
    async fn test_load_invalid_json() {
        let (repo, _temp) = create_test_repo();

        // Write invalid JSON
        let jobs_path = repo.path();
        fs::write(jobs_path, b"invalid json").await.unwrap();

        let result = repo.load().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            JobRepoError::JsonParse(_) => {}
            _ => panic!("Expected JsonParse error"),
        }
    }

    #[tokio::test]
    async fn test_job_repo_error_display() {
        let err = JobRepoError::NotFound("job1".to_string());
        assert_eq!(err.to_string(), "Job not found: job1");

        let err = JobRepoError::Io("Failed to read".to_string());
        assert_eq!(err.to_string(), "IO error: Failed to read");

        let err = JobRepoError::JsonParse("Invalid JSON".to_string());
        assert_eq!(err.to_string(), "JSON parse error: Invalid JSON");
    }
}
