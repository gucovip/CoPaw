// -*- coding: utf-8 -*-
// Environment variable store for loading and saving envs.json

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;
use tracing::{debug, warn};

/// Get the default envs.json path
fn get_envs_json_path() -> PathBuf {
    // Default to {working_dir}/envs.json; if caller provides a .json path, respect it.
    let raw = std::env::var("COPAW_WORKING_DIR").unwrap_or_else(|_| "~/.copaw".to_string());
    let expanded = raw.replace(
        '~',
        &std::env::var("HOME").unwrap_or_else(|_| ".".to_string()),
    );
    let base: PathBuf = expanded.into();

    if base
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
    {
        base
    } else {
        base.join("envs.json")
    }
}

/// Errors for environment variable operations
#[derive(Debug, Error)]
pub enum EnvError {
    #[error("Environment variable not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid key: {0}")]
    InvalidKey(String),
}

/// Mask environment variable value for security
pub fn mask_env_value(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }

    let length = value.len();
    if length <= 8 {
        // For short values, just mask everything
        return "*".repeat(length);
    }

    // Show first 2-3 characters (3 if there's a dash at position 2)
    let chars: Vec<char> = value.chars().collect();
    let prefix_len = if length > 2 && chars.get(2) == Some(&'-') {
        3
    } else {
        2
    };

    let prefix: String = chars.iter().take(prefix_len).collect();
    let suffix: String = chars
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    // Calculate masked section length (at least 4 asterisks)
    let masked_len = std::cmp::max(length - prefix_len - 4, 4);

    format!("{}{}{}", prefix, "*".repeat(masked_len), suffix)
}

/// Environment variable store
pub struct EnvStore {
    path: PathBuf,
}

impl EnvStore {
    /// Create a new env store with the given path
    pub fn new(path: Option<PathBuf>) -> Self {
        let resolved = match path {
            Some(p) if p.is_dir() => p.join("envs.json"),
            Some(p) => p,
            None => get_envs_json_path(),
        };

        Self { path: resolved }
    }

    /// Get the envs.json file path
    pub fn get_path(&self) -> &Path {
        &self.path
    }

    /// Load environment variables from envs.json
    pub async fn load(&self) -> Result<HashMap<String, String>, EnvError> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }

        let content = fs::read_to_string(&self.path).await?;

        if content.trim().is_empty() {
            return Ok(HashMap::new());
        }

        let data: serde_json::Value = serde_json::from_str(&content)?;

        if !data.is_object() {
            warn!("envs.json is not an object, returning empty map");
            return Ok(HashMap::new());
        }

        let envs: HashMap<String, String> = data
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        debug!("Loaded {} environment variables from envs.json", envs.len());
        Ok(envs)
    }

    /// Save environment variables to envs.json
    pub async fn save(&self, envs: &HashMap<String, String>) -> Result<(), EnvError> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(envs)?;
        fs::write(&self.path, content).await?;

        debug!("Saved {} environment variables to envs.json", envs.len());
        Ok(())
    }

    /// Set a single environment variable
    pub async fn set(&self, key: &str, value: &str) -> Result<HashMap<String, String>, EnvError> {
        let trimmed_key = key.trim();
        if trimmed_key.is_empty() {
            return Err(EnvError::InvalidKey("Key cannot be empty".to_string()));
        }

        let mut envs = self.load().await?;
        envs.insert(trimmed_key.to_string(), value.to_string());
        self.save(&envs).await?;

        Ok(envs)
    }

    /// Delete a single environment variable
    pub async fn delete(&self, key: &str) -> Result<HashMap<String, String>, EnvError> {
        let mut envs = self.load().await?;

        if !envs.contains_key(key) {
            return Err(EnvError::NotFound(key.to_string()));
        }

        envs.remove(key);
        self.save(&envs).await?;

        Ok(envs)
    }

    /// Batch save environment variables (replaces all)
    pub async fn batch_save(
        &self,
        envs: HashMap<String, String>,
    ) -> Result<HashMap<String, String>, EnvError> {
        // Validate keys
        for key in envs.keys() {
            let trimmed = key.trim();
            if trimmed.is_empty() {
                return Err(EnvError::InvalidKey("Key cannot be empty".to_string()));
            }
        }

        // Clean the keys
        let cleaned: HashMap<String, String> = envs
            .into_iter()
            .map(|(k, v)| (k.trim().to_string(), v))
            .collect();

        self.save(&cleaned).await?;
        Ok(cleaned)
    }

    /// List all environment variables with masked values
    pub async fn list_masked(&self) -> Result<Vec<(String, String)>, EnvError> {
        let envs = self.load().await?;
        let masked: Vec<(String, String)> = envs
            .into_iter()
            .map(|(k, v)| (k, mask_env_value(&v)))
            .collect();

        Ok(masked)
    }
}

impl Default for EnvStore {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_mask_env_value() {
        assert_eq!(mask_env_value(""), "");
        assert_eq!(mask_env_value("short"), "*****");
        assert_eq!(
            mask_env_value("sk-proj-1234567890abcdefghij1234"),
            "sk-*************************1234"
        );
        assert_eq!(mask_env_value("my-api-key-value"), "my-*********alue");
        assert_eq!(mask_env_value("ab123456789xyz"), "ab********9xyz");
    }

    #[tokio::test]
    async fn test_env_store_load_empty() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("envs.json");
        let store = EnvStore::new(Some(path));

        let envs = store.load().await.unwrap();
        assert!(envs.is_empty());
    }

    #[tokio::test]
    async fn test_env_store_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("envs.json");
        let store = EnvStore::new(Some(path));

        let mut envs = HashMap::new();
        envs.insert("TEST_KEY".to_string(), "test_value".to_string());
        store.save(&envs).await.unwrap();

        let loaded = store.load().await.unwrap();
        assert_eq!(loaded.get("TEST_KEY"), Some(&"test_value".to_string()));
    }

    #[tokio::test]
    async fn test_env_store_set() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("envs.json");
        let store = EnvStore::new(Some(path));

        let envs = store.set("NEW_KEY", "new_value").await.unwrap();
        assert_eq!(envs.get("NEW_KEY"), Some(&"new_value".to_string()));

        let loaded = store.load().await.unwrap();
        assert_eq!(loaded.get("NEW_KEY"), Some(&"new_value".to_string()));
    }

    #[tokio::test]
    async fn test_env_store_delete() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("envs.json");
        let store = EnvStore::new(Some(path));

        // First set a value
        store.set("TO_DELETE", "value").await.unwrap();

        // Then delete it
        let envs = store.delete("TO_DELETE").await.unwrap();
        assert!(!envs.contains_key("TO_DELETE"));

        // Try to delete again - should fail
        let result = store.delete("TO_DELETE").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_env_store_batch_save() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("envs.json");
        let store = EnvStore::new(Some(path));

        let mut envs = HashMap::new();
        envs.insert("KEY1".to_string(), "value1".to_string());
        envs.insert("KEY2".to_string(), "value2".to_string());

        let result = store.batch_save(envs.clone()).await.unwrap();
        assert_eq!(result.len(), 2);

        let loaded = store.load().await.unwrap();
        assert_eq!(loaded.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(loaded.get("KEY2"), Some(&"value2".to_string()));
    }

    #[tokio::test]
    async fn test_env_store_list_masked() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("envs.json");
        let store = EnvStore::new(Some(path));

        store
            .set("API_KEY", "sk-proj-1234567890abcdefghij1234")
            .await
            .unwrap();

        let masked = store.list_masked().await.unwrap();
        let (_, masked_value) = masked.iter().find(|(k, _)| k == "API_KEY").unwrap();

        assert!(!masked_value.contains("sk-proj-1234567890abcdefghij1234"));
        assert!(masked_value.contains('*'));
    }

    #[tokio::test]
    async fn test_env_store_invalid_key() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("envs.json");
        let store = EnvStore::new(Some(path));

        let result = store.set("", "value").await;
        assert!(result.is_err());

        let mut envs = HashMap::new();
        envs.insert("".to_string(), "value".to_string());
        let result = store.batch_save(envs).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_env_store_new_with_directory_path() {
        let temp_dir = TempDir::new().unwrap();
        let store = EnvStore::new(Some(temp_dir.path().to_path_buf()));

        assert_eq!(store.get_path(), &temp_dir.path().join("envs.json"));

        let envs = store.set("DIR_MODE", "ok").await.unwrap();
        assert_eq!(envs.get("DIR_MODE"), Some(&"ok".to_string()));
    }
}
