use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP transport type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    Stdio,
    #[serde(alias = "streamablehttp")]
    #[serde(alias = "http")]
    StreamableHttp,
    Sse,
}

impl Default for TransportType {
    fn default() -> Self {
        Self::Stdio
    }
}

impl TransportType {
    /// Normalize transport type from various string representations
    pub fn normalize(input: &str) -> Self {
        let normalized = input.trim().to_lowercase();
        match normalized.as_str() {
            "stdio" => Self::Stdio,
            "streamablehttp" | "http" | "streamable_http" => Self::StreamableHttp,
            "sse" => Self::Sse,
            _ => Self::Stdio,
        }
    }
}

/// Configuration for a single MCP client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MCPClientConfig {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub transport: TransportType,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub cwd: String,
}

impl Default for MCPClientConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            enabled: true,
            transport: TransportType::Stdio,
            url: String::new(),
            headers: HashMap::new(),
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: String::new(),
        }
    }
}

/// MCP clients configuration
/// Uses a map to allow dynamic client definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MCPConfig {
    #[serde(default)]
    pub clients: HashMap<String, MCPClientConfig>,
}

impl Default for MCPConfig {
    fn default() -> Self {
        let mut clients = HashMap::new();

        // Default tavily_search client (auto-enabled if TAVILY_API_KEY exists)
        let tavily_api_key = std::env::var("TAVILY_API_KEY").ok();
        let tavily_enabled = tavily_api_key.is_some();

        let tavily_config = MCPClientConfig {
            name: "tavily_mcp".to_string(),
            description: String::new(),
            enabled: tavily_enabled,
            transport: TransportType::Stdio,
            url: String::new(),
            headers: HashMap::new(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "tavily-mcp@latest".to_string()],
            env: if let Some(key) = tavily_api_key {
                [("TAVILY_API_KEY".to_string(), key)].into_iter().collect()
            } else {
                HashMap::new()
            },
            cwd: String::new(),
        };

        clients.insert("tavily_search".to_string(), tavily_config);

        Self { clients }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_type_normalize() {
        assert_eq!(TransportType::normalize("STDIO"), TransportType::Stdio);
        assert_eq!(
            TransportType::normalize("streamablehttp"),
            TransportType::StreamableHttp
        );
        assert_eq!(
            TransportType::normalize("HTTP"),
            TransportType::StreamableHttp
        );
        assert_eq!(TransportType::normalize("sse"), TransportType::Sse);
    }

    #[test]
    fn test_mcp_client_default() {
        let config = MCPClientConfig::default();
        assert_eq!(config.enabled, true);
        assert_eq!(config.transport, TransportType::Stdio);
    }

    #[test]
    fn test_mcp_config_serialization() {
        let config = MCPConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: MCPConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.clients.contains_key("tavily_search"));
    }
}
