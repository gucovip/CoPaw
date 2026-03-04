pub mod channels;
pub mod config;
pub mod heartbeat;
pub mod mcp;
pub mod utils;

pub use channels::{
    ChannelConfig, ConsoleConfig, DingTalkConfig, DiscordConfig, FeishuConfig, QQConfig,
    TelegramConfig,
};
pub use config::{
    AgentsConfig, AgentsDefaultsConfig, AgentsRunningConfig, CoPawConfig, LastApiConfig,
    LastDispatchConfig,
};
pub use heartbeat::{ActiveHoursConfig, HeartbeatConfig};
pub use mcp::{MCPClientConfig, MCPConfig, TransportType};
pub use utils::{
    expand_home, get_chats_path, get_jobs_path, get_media_path, get_skills_path, get_working_dir,
};

use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Configuration validation error: {0}")]
    Validation(String),
    #[error("Config watch error: {0}")]
    Watch(String),
}

/// Get the default configuration file path
/// ~/.copaw/config.json
pub fn get_config_path() -> PathBuf {
    let home_dir = dirs::home_dir().expect("Cannot get home directory");
    home_dir.join(".copaw/config.json")
}

/// Load configuration from file
///
/// # Arguments
/// * `path` - Path to config file (if None, uses default)
///
/// # Returns
/// Result containing the loaded configuration or error
pub async fn load_config(path: Option<&PathBuf>) -> Result<CoPawConfig, ConfigError> {
    let config_path = path.cloned().unwrap_or_else(get_config_path);

    let content = match tokio::fs::read_to_string(&config_path).await {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // On first run, config file may not exist yet.
            return Ok(CoPawConfig::default());
        }
        Err(err) => return Err(ConfigError::Io(err)),
    };

    let config: CoPawConfig = serde_json::from_str(&content)?;

    Ok(config)
}

/// Save configuration to file
///
/// # Arguments
/// * `config` - Configuration to save
/// * `path` - Path to save to (if None, uses default)
///
/// # Returns
/// Result indicating success or error
pub async fn save_config(config: &CoPawConfig, path: Option<&PathBuf>) -> Result<(), ConfigError> {
    let config_path = path.cloned().unwrap_or_else(get_config_path);

    // Ensure directory exists
    if let Some(parent) = config_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let content = serde_json::to_string_pretty(config)?;
    tokio::fs::write(&config_path, content).await?;

    Ok(())
}

/// Configuration change notification
#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub config: CoPawConfig,
}

/// Watch for config file changes and send notifications
///
/// This creates a file watcher that will send notifications
/// when the config file changes.
pub async fn watch_config(
    path: Option<&PathBuf>,
) -> Result<mpsc::UnboundedReceiver<ConfigChange>, ConfigError> {
    use notify::{RecursiveMode, Watcher};

    let config_path = path.cloned().unwrap_or_else(get_config_path);

    let (tx, rx) = mpsc::unbounded_channel();

    let config_path_for_watch = config_path.clone();
    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if event.kind.is_modify() || event.kind.is_create() {
                        // Spawn a task to reload config
                        let config_path = config_path_for_watch.to_path_buf();
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            if let Ok(config) = load_config(Some(&config_path)).await {
                                let _ = tx.send(ConfigChange { config });
                            }
                        });
                    }
                }
                Err(e) => eprintln!("Config watch error: {}", e),
            }
        })
        .map_err(|e| ConfigError::Watch(e.to_string()))?;

    watcher
        .watch(&config_path, RecursiveMode::NonRecursive)
        .map_err(|e| ConfigError::Watch(e.to_string()))?;

    // Keep the watcher alive in a background task
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_config_path() {
        let path = get_config_path();
        assert_eq!(path.extension().and_then(|s| s.to_str()), Some("json"));
        assert_eq!(
            path.parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str()),
            Some(".copaw")
        );
    }

    #[tokio::test]
    async fn test_save_config() {
        let mut config = CoPawConfig::default();
        config.agents.language = "en".to_string();

        let temp_path = std::env::temp_dir().join("test_config.json");

        let result = save_config(&config, Some(&temp_path)).await;
        assert!(result.is_ok());

        // Verify file was created
        assert!(tokio::fs::try_exists(&temp_path).await.unwrap());

        // Load and verify
        let loaded = load_config(Some(&temp_path)).await.unwrap();
        assert_eq!(loaded.agents.language, "en");

        // Clean up
        let _ = tokio::fs::remove_file(&temp_path).await;
    }

    #[tokio::test]
    async fn test_load_config_missing_returns_default() {
        let temp_path =
            std::env::temp_dir().join(format!("copaw-missing-config-{}.json", std::process::id()));

        // Ensure file does not exist
        let _ = tokio::fs::remove_file(&temp_path).await;

        let loaded = load_config(Some(&temp_path)).await.unwrap();
        assert_eq!(loaded.show_tool_details, true);
        assert_eq!(loaded.agents.language, "zh");
        assert!(loaded.mcp.clients.contains_key("tavily_search"));
    }
}
