// -*- coding: utf-8 -*-
// API request/response schemas for providers/models, chats, and agent endpoints.

use copaw_providers::{ModelInfo, ModelSlotConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Provider configuration request.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfigRequest {
    /// API key to configure.
    pub api_key: Option<String>,
    /// Base URL to configure.
    pub base_url: Option<String>,
}

/// Model slot request.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelSlotRequest {
    /// Provider to use.
    pub provider_id: String,
    /// Model identifier.
    pub model: String,
}

/// Create custom provider request.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateCustomProviderRequest {
    /// Provider ID.
    pub id: String,
    /// Provider name.
    pub name: String,
    /// Default base URL.
    #[serde(default)]
    pub default_base_url: String,
    /// API key prefix.
    #[serde(default)]
    pub api_key_prefix: String,
    /// Models list.
    #[serde(default)]
    pub models: Vec<ModelInfo>,
}

/// Add model request.
#[derive(Debug, Clone, Deserialize)]
pub struct AddModelRequest {
    /// Model ID.
    pub id: String,
    /// Model name.
    pub name: String,
}

impl From<AddModelRequest> for ModelInfo {
    fn from(req: AddModelRequest) -> Self {
        ModelInfo {
            id: req.id,
            name: req.name,
        }
    }
}

/// Test provider request.
#[derive(Debug, Clone, Deserialize)]
pub struct TestProviderRequest {
    /// Optional API key to test.
    pub api_key: Option<String>,
    /// Optional base URL to test.
    pub base_url: Option<String>,
}

/// Test model request.
#[derive(Debug, Clone, Deserialize)]
pub struct TestModelRequest {
    /// Model ID to test.
    pub model_id: String,
}

/// Test connection response.
#[derive(Debug, Clone, Serialize)]
pub struct TestConnectionResponse {
    /// Whether the test passed.
    pub success: bool,
    /// Human-readable result message.
    pub message: String,
}

/// Provider info returned by API.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    /// Provider ID.
    pub id: String,
    /// Provider name.
    pub name: String,
    /// API key prefix.
    pub api_key_prefix: String,
    /// Available models.
    #[serde(default)]
    pub models: Vec<ModelInfo>,
    /// Extra models added by user.
    #[serde(default)]
    pub extra_models: Vec<ModelInfo>,
    /// Whether this is a custom provider.
    #[serde(default)]
    pub is_custom: bool,
    /// Whether this is a local provider.
    #[serde(default)]
    pub is_local: bool,
    /// Whether user must supply a base URL.
    #[serde(default)]
    pub needs_base_url: bool,
    /// Whether provider has an API key configured.
    #[serde(default)]
    pub has_api_key: bool,
    /// Current API key (masked).
    #[serde(default)]
    pub current_api_key: String,
    /// Current base URL.
    #[serde(default)]
    pub current_base_url: String,
}

/// Active models info.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveModelsInfo {
    /// Active LLM configuration.
    #[serde(rename = "active_llm")]
    pub active_llm: ModelSlotConfigPublic,
}

/// Public version of ModelSlotConfig for API responses.
#[derive(Debug, Clone, Serialize)]
pub struct ModelSlotConfigPublic {
    /// Provider ID.
    pub provider_id: String,
    /// Model identifier.
    pub model: String,
}

impl From<ModelSlotConfig> for ModelSlotConfigPublic {
    fn from(slot: ModelSlotConfig) -> Self {
        Self {
            provider_id: slot.provider_id,
            model: slot.model,
        }
    }
}

impl From<&ModelSlotConfig> for ModelSlotConfigPublic {
    fn from(slot: &ModelSlotConfig) -> Self {
        Self {
            provider_id: slot.provider_id.clone(),
            model: slot.model.clone(),
        }
    }
}

// ============================================================================
// Chat Schemas
// ============================================================================

/// Chat specification with UUID identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSpec {
    /// Chat UUID identifier
    #[serde(default)]
    pub id: String,
    /// Chat name
    #[serde(default = "default_chat_name")]
    pub name: String,
    /// Session identifier (channel:user_id format)
    pub session_id: String,
    /// User identifier
    pub user_id: String,
    /// Channel name
    #[serde(default = "default_channel")]
    pub channel: String,
    /// Chat creation timestamp (RFC3339)
    #[serde(default)]
    pub created_at: String,
    /// Chat last update timestamp (RFC3339)
    #[serde(default)]
    pub updated_at: String,
    /// Additional metadata
    #[serde(default)]
    pub meta: HashMap<String, serde_json::Value>,
}

fn default_chat_name() -> String {
    "New Chat".to_string()
}

fn default_channel() -> String {
    "console".to_string()
}

/// Chat file for JSON repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatsFile {
    /// File format version
    #[serde(default)]
    pub version: i32,
    /// List of chat specs
    #[serde(default)]
    pub chats: Vec<ChatSpec>,
}

impl Default for ChatsFile {
    fn default() -> Self {
        Self {
            version: 1,
            chats: Vec::new(),
        }
    }
}

/// Chat list query parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct ChatListQuery {
    /// Filter by channel
    pub channel: Option<String>,
    /// Filter by user ID
    pub user_id: Option<String>,
    /// Filter by session ID
    pub session_id: Option<String>,
}

/// Create chat request.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateChatRequest {
    /// Chat name
    #[serde(default = "default_chat_name")]
    pub name: String,
    /// Session identifier
    pub session_id: String,
    /// User identifier
    pub user_id: String,
    /// Channel name
    #[serde(default = "default_channel")]
    pub channel: String,
}

/// Update chat request.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateChatRequest {
    /// Chat name
    pub name: Option<String>,
    /// Additional metadata
    #[serde(default)]
    pub meta: HashMap<String, serde_json::Value>,
}

/// Batch delete request.
#[derive(Debug, Clone, Deserialize)]
pub struct BatchDeleteRequest {
    /// List of chat IDs to delete
    pub ids: Vec<String>,
}

/// Chat history with messages.
#[derive(Debug, Clone, Serialize)]
pub struct ChatHistory {
    /// Chat messages
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
}

/// Chat message (simplified from AgentScope Message).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Message role (user, assistant, system)
    pub role: String,
    /// Message content
    pub content: String,
    /// Optional message name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

// ============================================================================
// Agent Schemas
// ============================================================================

/// Markdown file metadata.
#[derive(Debug, Clone, Serialize)]
pub struct MdFileInfo {
    /// File name
    pub filename: String,
    /// File path
    pub path: String,
    /// Size in bytes
    pub size: i64,
    /// Created time (ISO8601)
    pub created_time: String,
    /// Modified time (ISO8601)
    pub modified_time: String,
}

/// Markdown file content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MdFileContent {
    /// File content
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_model_request_to_model_info() {
        let req = AddModelRequest {
            id: "gpt-4".to_string(),
            name: "GPT-4".to_string(),
        };
        let info: ModelInfo = req.into();
        assert_eq!(info.id, "gpt-4");
        assert_eq!(info.name, "GPT-4");
    }

    #[test]
    fn test_provider_config_request_all_optional() {
        let json = r#"{}"#;
        let req: ProviderConfigRequest = serde_json::from_str(json).unwrap();
        assert!(req.api_key.is_none());
        assert!(req.base_url.is_none());
    }

    #[test]
    fn test_model_slot_request() {
        let json = r#"{"provider_id":"openai","model":"gpt-4"}"#;
        let req: ModelSlotRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.provider_id, "openai");
        assert_eq!(req.model, "gpt-4");
    }

    #[test]
    fn test_create_custom_provider_request() {
        let json = r#"{
            "id":"my-provider",
            "name":"My Provider",
            "default_base_url":"https://api.example.com/v1",
            "api_key_prefix":"custom-",
            "models":[]
        }"#;
        let req: CreateCustomProviderRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, "my-provider");
        assert_eq!(req.name, "My Provider");
        assert_eq!(req.default_base_url, "https://api.example.com/v1");
        assert_eq!(req.api_key_prefix, "custom-");
        assert!(req.models.is_empty());
    }

    #[test]
    fn test_test_connection_response() {
        let resp = TestConnectionResponse {
            success: true,
            message: "Connection successful".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("Connection successful"));
    }

    #[test]
    fn test_chat_spec_default_name() {
        let chat = ChatSpec {
            id: "test-id".to_string(),
            session_id: "test-session".to_string(),
            user_id: "test-user".to_string(),
            ..Default::default()
        };
        assert_eq!(chat.name, "New Chat");
        assert_eq!(chat.channel, "console");
    }

    #[test]
    fn test_chats_file_default() {
        let file = ChatsFile::default();
        assert_eq!(file.version, 1);
        assert!(file.chats.is_empty());
    }

    #[test]
    fn test_chat_list_query_all_optional() {
        let json = r#"{}"#;
        let query: ChatListQuery = serde_json::from_str(json).unwrap();
        assert!(query.channel.is_none());
        assert!(query.user_id.is_none());
        assert!(query.session_id.is_none());
    }

    #[test]
    fn test_create_chat_request() {
        let json = r#"{
            "name": "My Chat",
            "session_id": "console:test-user",
            "user_id": "test-user",
            "channel": "console"
        }"#;
        let req: CreateChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "My Chat");
        assert_eq!(req.session_id, "console:test-user");
        assert_eq!(req.user_id, "test-user");
        assert_eq!(req.channel, "console");
    }

    #[test]
    fn test_update_chat_request() {
        let json = r#"{"name": "Updated Name"}"#;
        let req: UpdateChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("Updated Name".to_string()));
        assert!(req.meta.is_empty());
    }

    #[test]
    fn test_batch_delete_request() {
        let json = r#"{"ids": ["id1", "id2"]}"#;
        let req: BatchDeleteRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.ids.len(), 2);
        assert_eq!(req.ids[0], "id1");
    }

    #[test]
    fn test_md_file_content() {
        let json = "{\"content\": \"# Test Content\"}";
        let req: MdFileContent = serde_json::from_str(json).unwrap();
        assert_eq!(req.content, "# Test Content");
    }

    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            name: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
    }
}

impl Default for ChatSpec {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: default_chat_name(),
            session_id: String::new(),
            user_id: String::new(),
            channel: default_channel(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            meta: HashMap::new(),
        }
    }
}

// ============================================================================
// Skill Schemas
// ============================================================================

/// Skill specification with enabled flag
#[derive(Debug, Clone, Serialize)]
pub struct SkillSpec {
    pub name: String,
    pub content: String,
    pub source: String,
    pub path: String,
    #[serde(default)]
    pub references: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub scripts: HashMap<String, serde_json::Value>,
    pub enabled: bool,
}

/// Create skill request
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSkillRequest {
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub references: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub scripts: Option<HashMap<String, serde_json::Value>>,
}

/// Hub skill specification
#[derive(Debug, Clone, Serialize)]
pub struct HubSkillSpec {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub source_url: String,
}

/// Hub install request
#[derive(Debug, Clone, Deserialize)]
pub struct HubInstallRequest {
    pub bundle_url: String,
    #[serde(default)]
    pub version: String,
    #[serde(default = "default_enable")]
    pub enable: bool,
    #[serde(default)]
    pub overwrite: bool,
}

fn default_enable() -> bool {
    true
}

/// Batch enable/disable request
#[derive(Debug, Clone, Deserialize)]
pub struct BatchSkillsRequest {
    pub skill_names: Vec<String>,
}

/// Skill file content response
#[derive(Debug, Clone, Serialize)]
pub struct SkillFileContent {
    pub content: String,
}

// ============================================================================
// MCP Schemas
// ============================================================================

/// MCP client create request
#[derive(Debug, Clone, Deserialize)]
pub struct McpClientCreateRequest {
    pub client_key: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_enable")]
    pub enabled: bool,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: String,
}

fn default_transport() -> String {
    "stdio".to_string()
}

/// MCP client update request
#[derive(Debug, Clone, Deserialize)]
pub struct McpClientUpdateRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub cwd: Option<String>,
}

// ============================================================================
// Environment Schemas
// ============================================================================

/// Environment variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

// ============================================================================
// Cron Schemas
// ============================================================================

/// Job list response
#[derive(Debug, Clone, Serialize)]
pub struct JobListResponse {
    pub jobs: Vec<crate::crons::CronJobSpec>,
}

/// Create job response
#[derive(Debug, Clone, Serialize)]
pub struct CreateJobResponse {
    pub job: crate::crons::CronJobSpec,
}

/// Job pause response
#[derive(Debug, Clone, Serialize)]
pub struct JobPauseResponse {
    pub paused: bool,
}

/// Job resume response
#[derive(Debug, Clone, Serialize)]
pub struct JobResumeResponse {
    pub resumed: bool,
}

/// Job run response
#[derive(Debug, Clone, Serialize)]
pub struct JobRunResponse {
    pub started: bool,
}

/// Job state response
#[derive(Debug, Clone, Serialize)]
pub struct CronJobStateResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<crate::crons::JobStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}
