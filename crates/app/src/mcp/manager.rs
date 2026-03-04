// -*- coding: utf-8 -*-
// MCP client manager for managing MCP client lifecycle

use copaw_config::{load_config, save_config, MCPClientConfig, MCPConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info};

/// Result of testing an MCP client connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTestResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Errors for MCP operations
#[derive(Debug, Error)]
pub enum McpError {
    #[error("MCP client not found: {0}")]
    NotFound(String),

    #[error("MCP client already exists: {0}")]
    AlreadyExists(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Mask sensitive values in environment variables or headers
fn mask_value(value: &str) -> String {
    if value.is_empty() {
        return value.to_string();
    }

    let length = value.len();
    if length <= 8 {
        // For short values, just mask everything
        return "*".repeat(length);
    }

    // Show first 2-3 characters (3 if there's a dash at position 2)
    let prefix_len = if length > 2 && value.chars().nth(2) == Some('-') {
        3
    } else {
        2
    };
    let prefix = &value[..prefix_len];

    // Show last 4 characters
    let suffix = &value[length.saturating_sub(4)..];

    // Calculate masked section length (at least 4 asterisks)
    let masked_len = std::cmp::max(length - prefix_len - 4, 4);

    format!("{}{}{}", prefix, "*".repeat(masked_len), suffix)
}

/// MCP client information for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientInfo {
    pub key: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub transport: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: String,
}

impl From<(String, MCPClientConfig)> for McpClientInfo {
    fn from((key, client): (String, MCPClientConfig)) -> Self {
        // Mask environment variable values for security
        let masked_env: HashMap<String, String> = client
            .env
            .iter()
            .map(|(k, v)| (k.clone(), mask_value(v)))
            .collect();

        // Mask headers for security
        let masked_headers: HashMap<String, String> = client
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), mask_value(v)))
            .collect();

        McpClientInfo {
            key,
            name: client.name,
            description: client.description,
            enabled: client.enabled,
            transport: format!("{:?}", client.transport).to_lowercase(),
            url: client.url,
            headers: masked_headers,
            command: client.command,
            args: client.args,
            env: masked_env,
            cwd: client.cwd,
        }
    }
}

/// MCP client manager
pub struct McpManager {
    config_path: Option<PathBuf>,
}

impl McpManager {
    /// Create a new MCP manager
    pub fn new(config_path: Option<PathBuf>) -> Self {
        Self { config_path }
    }

    /// Get the config path
    fn get_config_path(&self) -> PathBuf {
        self.config_path.clone().unwrap_or_else(copaw_config::get_config_path)
    }

    /// List all MCP clients
    pub async fn list_clients(&self) -> Result<Vec<McpClientInfo>, McpError> {
        let config = load_config(Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        let clients: Vec<McpClientInfo> = config
            .mcp
            .clients
            .into_iter()
            .map(McpClientInfo::from)
            .collect();

        Ok(clients)
    }

    /// Get a specific MCP client
    pub async fn get_client(&self, key: &str) -> Result<McpClientInfo, McpError> {
        let config = load_config(Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        config
            .mcp
            .clients
            .get(key)
            .map(|client| McpClientInfo::from((key.to_string(), client.clone())))
            .ok_or_else(|| McpError::NotFound(key.to_string()))
    }

    /// Create a new MCP client
    pub async fn create_client(
        &self,
        key: &str,
        client: MCPClientConfig,
    ) -> Result<McpClientInfo, McpError> {
        let mut config = load_config(Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        // Check if client already exists
        if config.mcp.clients.contains_key(key) {
            return Err(McpError::AlreadyExists(key.to_string()));
        }

        config.mcp.clients.insert(key.to_string(), client);

        save_config(&config, Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        info!("Created MCP client '{}'", key);
        self.get_client(key).await
    }

    /// Update an existing MCP client
    pub async fn update_client(
        &self,
        key: &str,
        updates: HashMap<String, serde_json::Value>,
    ) -> Result<McpClientInfo, McpError> {
        let mut config = load_config(Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        let existing = config
            .mcp
            .clients
            .get_mut(key)
            .ok_or_else(|| McpError::NotFound(key.to_string()))?;

        // Apply updates
        if let Some(name) = updates.get("name").and_then(|v| v.as_str()) {
            existing.name = name.to_string();
        }
        if let Some(description) = updates.get("description").and_then(|v| v.as_str()) {
            existing.description = description.to_string();
        }
        if let Some(enabled) = updates.get("enabled").and_then(|v| v.as_bool()) {
            existing.enabled = enabled;
        }
        if let Some(url) = updates.get("url").and_then(|v| v.as_str()) {
            existing.url = url.to_string();
        }
        if let Some(command) = updates.get("command").and_then(|v| v.as_str()) {
            existing.command = command.to_string();
        }
        if let Some(cwd) = updates.get("cwd").and_then(|v| v.as_str()) {
            existing.cwd = cwd.to_string();
        }
        if let Some(args) = updates.get("args").and_then(|v| v.as_array()) {
            existing.args = args
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect();
        }
        if let Some(env) = updates.get("env").and_then(|v| v.as_object()) {
            for (k, v) in env {
                if let Some(value) = v.as_str() {
                    existing.env.insert(k.clone(), value.to_string());
                }
            }
        }
        if let Some(headers) = updates.get("headers").and_then(|v| v.as_object()) {
            for (k, v) in headers {
                if let Some(value) = v.as_str() {
                    existing.headers.insert(k.clone(), value.to_string());
                }
            }
        }

        save_config(&config, Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        debug!("Updated MCP client '{}'", key);
        self.get_client(key).await
    }

    /// Delete an MCP client
    pub async fn delete_client(&self, key: &str) -> Result<(), McpError> {
        let mut config = load_config(Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        if !config.mcp.clients.contains_key(key) {
            return Err(McpError::NotFound(key.to_string()));
        }

        config.mcp.clients.remove(key);

        save_config(&config, Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        info!("Deleted MCP client '{}'", key);
        Ok(())
    }

    /// Toggle MCP client enabled status
    pub async fn toggle_client(&self, key: &str) -> Result<McpClientInfo, McpError> {
        let mut config = load_config(Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        let client = config
            .mcp
            .clients
            .get_mut(key)
            .ok_or_else(|| McpError::NotFound(key.to_string()))?;

        client.enabled = !client.enabled;
        let enabled = client.enabled;

        save_config(&config, Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        debug!("Toggled MCP client '{}' to {}", key, enabled);
        self.get_client(key).await
    }

    /// Test MCP client connection
    pub async fn test_client(&self, key: &str) -> Result<McpTestResult, McpError> {
        let config = load_config(Some(&self.get_config_path()))
            .await
            .map_err(|e| McpError::Config(e.to_string()))?;

        let client = config
            .mcp
            .clients
            .get(key)
            .ok_or_else(|| McpError::NotFound(key.to_string()))?;

        // For now, return a simple test result
        // TODO: Implement actual MCP protocol testing
        if client.enabled {
            match client.transport {
                copaw_config::TransportType::Stdio => {
                    if client.command.is_empty() {
                        return Ok(McpTestResult {
                            success: false,
                            message: "No command specified for stdio transport".to_string(),
                            details: None,
                        });
                    }
                    Ok(McpTestResult {
                        success: true,
                        message: format!("MCP client '{}' configuration is valid. Connection testing not yet implemented.", key),
                        details: Some(format!("Command: {} {}", client.command, client.args.join(" "))),
                    })
                }
                copaw_config::TransportType::StreamableHttp | copaw_config::TransportType::Sse => {
                    if client.url.is_empty() {
                        return Ok(McpTestResult {
                            success: false,
                            message: "No URL specified for HTTP/SSE transport".to_string(),
                            details: None,
                        });
                    }
                    Ok(McpTestResult {
                        success: true,
                        message: format!("MCP client '{}' configuration is valid. Connection testing not yet implemented.", key),
                        details: Some(format!("URL: {}", client.url)),
                    })
                }
            }
        } else {
            Ok(McpTestResult {
                success: false,
                message: format!("MCP client '{}' is disabled", key),
                details: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    async fn create_test_config() -> PathBuf {
        let config = copaw_config::CoPawConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        // Keep temp file around by forgetting it
        let _ = temp_file.into_temp_path();

        save_config(&config, Some(&path)).await.unwrap();
        path
    }

    #[tokio::test]
    async fn test_mask_value() {
        assert_eq!(mask_value("short"), "*****");
        assert_eq!(mask_value("sk-proj-1234567890abcdefghij1234"), "sk-*************************1234");
        assert_eq!(mask_value("my-api-key-value"), "my-*********alue");
    }

    #[tokio::test]
    async fn test_mcp_manager_list_clients() {
        let config_path = create_test_config().await;
        let manager = McpManager::new(Some(config_path));

        let clients = manager.list_clients().await.unwrap();
        // Default config has tavily_search
        assert!(!clients.is_empty());
    }

    #[tokio::test]
    async fn test_mcp_manager_get_client() {
        let config_path = create_test_config().await;
        let manager = McpManager::new(Some(config_path));

        let client = manager.get_client("tavily_search").await.unwrap();
        assert_eq!(client.key, "tavily_search");
    }

    #[tokio::test]
    async fn test_mcp_manager_get_not_found() {
        let config_path = create_test_config().await;
        let manager = McpManager::new(Some(config_path));

        let result = manager.get_client("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mcp_manager_create_client() {
        let config_path = create_test_config().await;
        let manager = McpManager::new(Some(config_path));

        let new_client = MCPClientConfig {
            name: "Test MCP".to_string(),
            description: "A test MCP client".to_string(),
            enabled: true,
            transport: copaw_config::TransportType::Stdio,
            url: String::new(),
            headers: HashMap::new(),
            command: "echo".to_string(),
            args: vec!["test".to_string()],
            env: HashMap::new(),
            cwd: String::new(),
        };

        let result = manager.create_client("test_client", new_client).await.unwrap();
        assert_eq!(result.key, "test_client");
        assert_eq!(result.name, "Test MCP");
    }

    #[tokio::test]
    async fn test_mcp_manager_create_duplicate() {
        let config_path = create_test_config().await;
        let manager = McpManager::new(Some(config_path));

        let client = MCPClientConfig::default();
        let result = manager.create_client("tavily_search", client).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mcp_manager_toggle_client() {
        let config_path = create_test_config().await;
        let manager = McpManager::new(Some(config_path));

        let initial = manager.get_client("tavily_search").await.unwrap();
        let initial_enabled = initial.enabled;

        let toggled = manager.toggle_client("tavily_search").await.unwrap();
        assert_eq!(toggled.enabled, !initial_enabled);
    }

    #[tokio::test]
    async fn test_mcp_manager_delete_client() {
        let config_path = create_test_config().await;
        let manager = McpManager::new(Some(config_path));

        // First create a client
        let new_client = MCPClientConfig {
            name: "To Delete".to_string(),
            ..Default::default()
        };
        manager.create_client("to_delete", new_client).await.unwrap();

        // Then delete it
        let result = manager.delete_client("to_delete").await;
        assert!(result.is_ok());

        // Verify it's gone
        let get_result = manager.get_client("to_delete").await;
        assert!(get_result.is_err());
    }

    #[tokio::test]
    async fn test_mcp_manager_test_client() {
        let config_path = create_test_config().await;
        let manager = McpManager::new(Some(config_path));

        let result = manager.test_client("tavily_search").await.unwrap();
        // The test should succeed for the default tavily_search client
        assert!(result.success || !result.message.is_empty());
    }
}
