// -*- coding: utf-8 -*-
// Async download manager for model downloads.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockWriteGuard};
use uuid::Uuid;

/// Download task status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

/// A download task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub task_id: String,
    pub repo_id: String,
    pub filename: Option<String>,
    pub backend: String,
    pub source: String,
    pub status: DownloadStatus,
    pub error: Option<String>,
    pub result: Option<DownloadResult>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub progress: f32,
}

/// Result of a completed download.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DownloadResult {
    pub id: String,
    pub repo_id: String,
    pub filename: String,
    pub backend: String,
    pub source: String,
    pub file_size: u64,
    pub local_path: String,
    pub display_name: String,
}

/// Download manager for tracking model downloads.
#[derive(Clone)]
pub struct DownloadManager {
    tasks: Arc<RwLock<HashMap<String, DownloadTask>>>,
    #[allow(dead_code)]
    models_dir: PathBuf,
}

impl DownloadManager {
    /// Create a new download manager.
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            models_dir,
        }
    }

    /// Get the models directory.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// Create a new download task.
    pub async fn create_task(
        &self,
        repo_id: String,
        filename: Option<String>,
        backend: String,
        source: String,
    ) -> DownloadTask {
        let task_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let task = DownloadTask {
            task_id: task_id.clone(),
            repo_id,
            filename,
            backend,
            source,
            status: DownloadStatus::Pending,
            error: None,
            result: None,
            created_at: now,
            updated_at: now,
            progress: 0.0,
        };

        self.tasks
            .write()
            .await
            .insert(task_id.clone(), task.clone());
        task
    }

    /// Get a task by ID.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn get_task(&self, task_id: &str) -> Option<DownloadTask> {
        self.tasks.read().await.get(task_id).cloned()
    }

    /// Get all tasks, optionally filtered by backend.
    pub async fn get_tasks(&self, backend: Option<&str>) -> Vec<DownloadTask> {
        let tasks = self.tasks.read().await;
        let tasks: Vec<DownloadTask> = tasks.values().cloned().collect();

        if let Some(backend) = backend {
            tasks.into_iter().filter(|t| t.backend == backend).collect()
        } else {
            tasks
        }
    }

    /// Update task status.
    pub async fn update_status(
        &self,
        task_id: &str,
        status: DownloadStatus,
    ) -> Option<DownloadTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks.get_mut(task_id)?;

        task.status = status;
        task.updated_at = Utc::now();

        Some(task.clone())
    }

    /// Update task with error.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn update_error(&self, task_id: &str, error: String) -> Option<DownloadTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks.get_mut(task_id)?;

        task.status = DownloadStatus::Failed;
        task.error = Some(error);
        task.updated_at = Utc::now();

        Some(task.clone())
    }

    /// Update task with result.
    pub async fn update_result(
        &self,
        task_id: &str,
        result: DownloadResult,
    ) -> Option<DownloadTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks.get_mut(task_id)?;

        task.status = DownloadStatus::Completed;
        task.result = Some(result);
        task.updated_at = Utc::now();
        task.progress = 100.0;

        Some(task.clone())
    }

    /// Update task progress.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn update_progress(&self, task_id: &str, progress: f32) -> Option<DownloadTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks.get_mut(task_id)?;

        task.progress = progress.clamp(0.0, 100.0);
        if task.progress >= 100.0 {
            task.status = DownloadStatus::Completed;
        }
        task.updated_at = Utc::now();

        Some(task.clone())
    }

    /// Cancel a task.
    pub async fn cancel_task(&self, task_id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        let task = tasks.get_mut(task_id);

        if let Some(task) = task {
            if matches!(
                task.status,
                DownloadStatus::Pending | DownloadStatus::Downloading
            ) {
                task.status = DownloadStatus::Cancelled;
                task.updated_at = Utc::now();
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Clear completed/failed/cancelled tasks.
    pub async fn clear_completed(&self, backend: Option<&str>) {
        let mut tasks = self.tasks.write().await;
        tasks.retain(|_, task| {
            let is_terminal = matches!(
                task.status,
                DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled
            );
            let matches_backend = backend.map_or(true, |b| &task.backend == b);

            !(is_terminal && matches_backend)
        });
    }

    /// Get a mutable reference to the tasks map.
    pub async fn tasks_mut(&self) -> RwLockWriteGuard<'_, HashMap<String, DownloadTask>> {
        self.tasks.write().await
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_manager() -> DownloadManager {
        DownloadManager::new(PathBuf::from("/tmp/test_models"))
    }

    #[tokio::test]
    async fn test_create_task() {
        let manager = create_test_manager();
        let task = manager
            .create_task(
                "test/repo".to_string(),
                Some("model.gguf".to_string()),
                "llamacpp".to_string(),
                "huggingface".to_string(),
            )
            .await;

        assert_eq!(task.status, DownloadStatus::Pending);
        assert_eq!(task.repo_id, "test/repo");
        assert_eq!(task.filename, Some("model.gguf".to_string()));
        assert_eq!(task.backend, "llamacpp");
        assert_eq!(task.source, "huggingface");
        assert!(task.error.is_none());
        assert!(task.result.is_none());
    }

    #[tokio::test]
    async fn test_get_task() {
        let manager = create_test_manager();
        let task = manager
            .create_task(
                "test/repo".to_string(),
                None,
                "llamacpp".to_string(),
                "huggingface".to_string(),
            )
            .await;

        let retrieved = manager.get_task(&task.task_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().task_id, task.task_id);
    }

    #[tokio::test]
    async fn test_get_tasks() {
        let manager = create_test_manager();

        manager
            .create_task(
                "repo1".to_string(),
                None,
                "llamacpp".to_string(),
                "huggingface".to_string(),
            )
            .await;
        manager
            .create_task(
                "repo2".to_string(),
                None,
                "mlx".to_string(),
                "huggingface".to_string(),
            )
            .await;

        let all_tasks = manager.get_tasks(None).await;
        assert_eq!(all_tasks.len(), 2);

        let llamacpp_tasks = manager.get_tasks(Some("llamacpp")).await;
        assert_eq!(llamacpp_tasks.len(), 1);
        assert_eq!(llamacpp_tasks[0].backend, "llamacpp");
    }

    #[tokio::test]
    async fn test_update_status() {
        let manager = create_test_manager();
        let task = manager
            .create_task(
                "test".to_string(),
                None,
                "llamacpp".to_string(),
                "huggingface".to_string(),
            )
            .await;

        let updated = manager
            .update_status(&task.task_id, DownloadStatus::Downloading)
            .await;
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().status, DownloadStatus::Downloading);

        let retrieved = manager.get_task(&task.task_id).await;
        assert_eq!(retrieved.unwrap().status, DownloadStatus::Downloading);
    }

    #[tokio::test]
    async fn test_update_error() {
        let manager = create_test_manager();
        let task = manager
            .create_task(
                "test".to_string(),
                None,
                "llamacpp".to_string(),
                "huggingface".to_string(),
            )
            .await;

        let error_msg = "Download failed".to_string();
        let updated = manager.update_error(&task.task_id, error_msg.clone()).await;

        assert!(updated.is_some());
        let updated_task = updated.unwrap();
        assert_eq!(updated_task.status, DownloadStatus::Failed);
        assert_eq!(updated_task.error, Some(error_msg));
    }

    #[tokio::test]
    async fn test_update_result() {
        let manager = create_test_manager();
        let task = manager
            .create_task(
                "test/repo".to_string(),
                Some("model.gguf".to_string()),
                "llamacpp".to_string(),
                "huggingface".to_string(),
            )
            .await;

        let result = DownloadResult {
            id: "test/repo/model.gguf".to_string(),
            repo_id: "test/repo".to_string(),
            filename: "model.gguf".to_string(),
            backend: "llamacpp".to_string(),
            source: "huggingface".to_string(),
            file_size: 1024,
            local_path: "/tmp/model.gguf".to_string(),
            display_name: "Model (model.gguf)".to_string(),
        };

        let updated = manager.update_result(&task.task_id, result.clone()).await;

        assert!(updated.is_some());
        let updated_task = updated.unwrap();
        assert_eq!(updated_task.status, DownloadStatus::Completed);
        assert_eq!(updated_task.result, Some(result));
        assert_eq!(updated_task.progress, 100.0);
    }

    #[tokio::test]
    async fn test_update_progress() {
        let manager = create_test_manager();
        let task = manager
            .create_task(
                "test".to_string(),
                None,
                "llamacpp".to_string(),
                "huggingface".to_string(),
            )
            .await;

        let updated = manager.update_progress(&task.task_id, 50.0).await;
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().progress, 50.0);

        // Test clamping
        let updated = manager.update_progress(&task.task_id, 150.0).await;
        assert_eq!(updated.unwrap().progress, 100.0);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let manager = create_test_manager();
        let task = manager
            .create_task(
                "test".to_string(),
                None,
                "llamacpp".to_string(),
                "huggingface".to_string(),
            )
            .await;

        // Cancel pending task
        let cancelled = manager.cancel_task(&task.task_id).await;
        assert!(cancelled);

        let retrieved = manager.get_task(&task.task_id).await;
        assert_eq!(retrieved.unwrap().status, DownloadStatus::Cancelled);

        // Try to cancel already cancelled task
        let cancelled_again = manager.cancel_task(&task.task_id).await;
        assert!(!cancelled_again);
    }

    #[tokio::test]
    async fn test_clear_completed() {
        let manager = create_test_manager();

        let task1 = manager
            .create_task(
                "repo1".to_string(),
                None,
                "llamacpp".to_string(),
                "huggingface".to_string(),
            )
            .await;
        manager
            .update_status(&task1.task_id, DownloadStatus::Completed)
            .await;

        let task2 = manager
            .create_task(
                "repo2".to_string(),
                None,
                "llamacpp".to_string(),
                "huggingface".to_string(),
            )
            .await;

        // Clear completed llamacpp tasks
        manager.clear_completed(Some("llamacpp")).await;

        let tasks = manager.get_tasks(None).await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, task2.task_id);
    }

    #[tokio::test]
    async fn test_default_manager() {
        let manager = DownloadManager::default();
        assert_eq!(manager.models_dir(), Path::new("."));
    }
}
