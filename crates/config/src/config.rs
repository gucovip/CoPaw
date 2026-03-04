use crate::channels::ChannelConfig;
use crate::heartbeat::HeartbeatConfig;
use crate::mcp::MCPConfig;
use serde::{Deserialize, Serialize};

/// Last API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LastApiConfig {
    pub host: Option<String>,
    pub port: Option<i32>,
}

impl Default for LastApiConfig {
    fn default() -> Self {
        Self {
            host: None,
            port: None,
        }
    }
}

/// Agent defaults configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsDefaultsConfig {
    pub heartbeat: Option<HeartbeatConfig>,
}

impl Default for AgentsDefaultsConfig {
    fn default() -> Self {
        Self { heartbeat: None }
    }
}

/// Agent runtime behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsRunningConfig {
    /// Maximum number of reasoning-acting iterations for ReAct agent
    pub max_iters: usize,
    /// Maximum input length (tokens) for model context window
    pub max_input_length: usize,
}

impl Default for AgentsRunningConfig {
    fn default() -> Self {
        Self {
            max_iters: 50,
            max_input_length: 128 * 1024, // 128K tokens
        }
    }
}

/// Agents configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    pub defaults: AgentsDefaultsConfig,
    pub running: AgentsRunningConfig,
    /// Language for agent MD files (en/zh)
    pub language: String,
    /// Language of currently installed md files
    pub installed_md_files_language: Option<String>,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            defaults: AgentsDefaultsConfig::default(),
            running: AgentsRunningConfig::default(),
            language: "zh".to_string(),
            installed_md_files_language: None,
        }
    }
}

/// Last channel/user/session that received a user-originated reply
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LastDispatchConfig {
    pub channel: String,
    pub user_id: String,
    pub session_id: String,
}

impl Default for LastDispatchConfig {
    fn default() -> Self {
        Self {
            channel: String::new(),
            user_id: String::new(),
            session_id: String::new(),
        }
    }
}

/// Root configuration (config.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CoPawConfig {
    pub channels: ChannelConfig,
    pub mcp: MCPConfig,
    pub last_api: LastApiConfig,
    pub agents: AgentsConfig,
    pub last_dispatch: Option<LastDispatchConfig>,
    /// When False, channel output hides tool call/result details (show "...")
    pub show_tool_details: bool,
}

impl Default for CoPawConfig {
    fn default() -> Self {
        Self {
            channels: ChannelConfig::default(),
            mcp: MCPConfig::default(),
            last_api: LastApiConfig::default(),
            agents: AgentsConfig::default(),
            last_dispatch: None,
            show_tool_details: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CoPawConfig::default();
        assert_eq!(config.show_tool_details, true);
        assert_eq!(config.agents.language, "zh");
    }

    #[test]
    fn test_agents_running_config() {
        let config = AgentsRunningConfig::default();
        assert_eq!(config.max_iters, 50);
        assert_eq!(config.max_input_length, 131072);
    }

    #[test]
    fn test_config_serialization() {
        let config = CoPawConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CoPawConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.show_tool_details, config.show_tool_details);
        assert_eq!(parsed.agents.language, config.agents.language);
    }
}
