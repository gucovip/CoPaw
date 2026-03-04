// -*- coding: utf-8 -*-
// -*- mode: rust -*-
/**
 * Core Message types for CoPaw agent communication.
 *
 * This module defines the message types used throughout the CoPaw system,
 * providing Rust equivalents to the Python AgentScope message types.
 *
 * Based on the Python implementation:
 * - agentscope.message.Msg
 * - agentscope.message.TextBlock
 * - agentscope.message.ImageBlock
 * - agentscope.message.ToolUseBlock
 * - agentscope.message.ThinkingBlock
 */
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Message role identifier
///
/// Corresponds to the `role` field in AgentScope Msg objects.
/// Valid roles are: system, user, assistant, tool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message (e.g., system prompts)
    System,
    /// User message
    User,
    /// Assistant response
    Assistant,
    /// Tool/function result
    Tool,
}

impl MessageRole {
    /// Get the string representation of the role
    pub fn as_str(&self) -> &str {
        match self {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        }
    }
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Content type for different message parts
///
/// Represents different types of content that can be in a message,
/// similar to AgentScope's content block types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Plain text content
    ///
    /// Corresponds to TextBlock in AgentScope
    Text { text: String },

    /// Image content with source
    ///
    /// Corresponds to ImageBlock in AgentScope
    Image { source: MediaSource },

    /// Audio content with source
    ///
    /// Corresponds to AudioBlock in AgentScope
    Audio { source: MediaSource },

    /// Video content with source
    ///
    /// Corresponds to VideoBlock in AgentScope
    Video { source: MediaSource },

    /// Tool/function use
    ///
    /// Corresponds to ToolUseBlock in AgentScope
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        raw_input: Option<String>,
    },

    /// Tool/function result
    ///
    /// Corresponds to ToolResultBlock in AgentScope
    ToolResult {
        id: String,
        name: String,
        output: ToolOutput,
    },

    /// Thinking/reasoning content
    ///
    /// Corresponds to ThinkingBlock in AgentScope
    Thinking { thinking: String },

    /// File content
    ///
    /// Corresponds to FileBlock in AgentScope
    File {
        source: MediaSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
    },
}

impl ContentPart {
    /// Create a new text content part
    pub fn text<S: Into<String>>(text: S) -> Self {
        ContentPart::Text { text: text.into() }
    }

    /// Create a new tool use content part
    pub fn tool_use<S: Into<String>>(id: S, name: S, input: serde_json::Value) -> Self {
        ContentPart::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
            raw_input: None,
        }
    }

    /// Create a new tool result content part
    pub fn tool_result<S: Into<String>>(id: S, name: S, output: ToolOutput) -> Self {
        ContentPart::ToolResult {
            id: id.into(),
            name: name.into(),
            output,
        }
    }

    /// Create a new thinking content part
    pub fn thinking<S: Into<String>>(thinking: S) -> Self {
        ContentPart::Thinking {
            thinking: thinking.into(),
        }
    }

    /// Get the content type as a string
    pub fn content_type(&self) -> &str {
        match self {
            ContentPart::Text { .. } => "text",
            ContentPart::Image { .. } => "image",
            ContentPart::Audio { .. } => "audio",
            ContentPart::Video { .. } => "video",
            ContentPart::ToolUse { .. } => "tool_use",
            ContentPart::ToolResult { .. } => "tool_result",
            ContentPart::Thinking { .. } => "thinking",
            ContentPart::File { .. } => "file",
        }
    }
}

/// Media source for image, audio, video content
///
/// Corresponds to URLSource and Base64Source in AgentScope
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MediaSource {
    /// URL-based media source
    Url { url: String },

    /// Base64-encoded media source
    Base64 { media_type: String, data: String },
}

/// Tool output can be a string or a list of content blocks
///
/// This matches the AgentScope ToolResponse output format
pub type ToolOutput = ToolOutputVariant;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolOutputVariant {
    /// Simple text output
    Text(String),

    /// Structured output with multiple content parts
    Blocks(Vec<ContentPart>),
}

impl ToolOutputVariant {
    /// Create a new text output
    pub fn text<S: Into<String>>(text: S) -> Self {
        ToolOutputVariant::Text(text.into())
    }

    /// Create a new blocks output
    pub fn blocks(blocks: Vec<ContentPart>) -> Self {
        ToolOutputVariant::Blocks(blocks)
    }
}

impl From<String> for ToolOutputVariant {
    fn from(s: String) -> Self {
        ToolOutputVariant::Text(s)
    }
}

impl From<&str> for ToolOutputVariant {
    fn from(s: &str) -> Self {
        ToolOutputVariant::Text(s.to_string())
    }
}

/// Message content representation
///
/// A message can have either simple string content (for backwards compatibility)
/// or structured content with multiple parts (for multimodal messages).
///
/// This corresponds to the flexible `content` field in AgentScope Msg objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple string content (legacy format)
    Text(String),

    /// Structured content with multiple parts
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    /// Create new text content
    pub fn text<S: Into<String>>(text: S) -> Self {
        MessageContent::Text(text.into())
    }

    /// Create new parts content
    pub fn parts(parts: Vec<ContentPart>) -> Self {
        MessageContent::Parts(parts)
    }

    /// Get text content if this is a simple text message
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(s) => Some(s),
            MessageContent::Parts(_) => None,
        }
    }

    /// Get parts if this is a parts message
    pub fn as_parts(&self) -> Option<&[ContentPart]> {
        match self {
            MessageContent::Text(_) => None,
            MessageContent::Parts(parts) => Some(parts),
        }
    }

    /// Get all content blocks (as a vec of ContentPart)
    pub fn to_parts(&self) -> Vec<ContentPart> {
        match self {
            MessageContent::Text(s) => {
                vec![ContentPart::text(s.clone())]
            }
            MessageContent::Parts(parts) => parts.clone(),
        }
    }
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        MessageContent::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        MessageContent::Text(s.to_string())
    }
}

impl From<Vec<ContentPart>> for MessageContent {
    fn from(parts: Vec<ContentPart>) -> Self {
        MessageContent::Parts(parts)
    }
}

/// Core message type
///
/// Represents a message in the CoPaw agent system.
/// This is the Rust equivalent of AgentScope's `Msg` class.
///
/// # Example
///
/// ```rust
/// use copaw_core::message::{Message, MessageRole, ContentPart};
///
/// // Create a simple user message
/// let msg = Message::user("Hello, world!");
///
/// // Create a message with structured content
/// let msg = Message::new(
///     MessageRole::User,
///     vec![ContentPart::text("Hello")]
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier
    id: String,

    /// Message role (system, user, assistant, tool)
    role: MessageRole,

    /// Message content
    content: MessageContent,

    /// Message metadata
    #[serde(default)]
    metadata: HashMap<String, serde_json::Value>,

    /// Message timestamp
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl Message {
    /// Create a new message with the given role and content
    pub fn new(role: MessageRole, content: impl Into<MessageContent>) -> Self {
        Message {
            id: uuid::Uuid::new_v4().to_string(),
            role,
            content: content.into(),
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a new system message
    pub fn system(content: impl Into<MessageContent>) -> Self {
        Message::new(MessageRole::System, content)
    }

    /// Create a new user message
    pub fn user(content: impl Into<MessageContent>) -> Self {
        Message::new(MessageRole::User, content)
    }

    /// Create a new assistant message
    pub fn assistant(content: impl Into<MessageContent>) -> Self {
        Message::new(MessageRole::Assistant, content)
    }

    /// Create a new tool message
    pub fn tool(content: impl Into<MessageContent>) -> Self {
        Message::new(MessageRole::Tool, content)
    }

    /// Get the message ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the message role
    pub fn role(&self) -> &MessageRole {
        &self.role
    }

    /// Get the message content
    pub fn content(&self) -> &MessageContent {
        &self.content
    }

    /// Get the message metadata
    pub fn metadata(&self) -> &HashMap<String, serde_json::Value> {
        &self.metadata
    }

    /// Get the message timestamp
    pub fn timestamp(&self) -> &chrono::DateTime<chrono::Utc> {
        &self.timestamp
    }

    /// Set metadata for the message
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get a specific metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }
}

/// Builder pattern for constructing messages
impl Message {
    /// Create a message builder
    pub fn builder() -> MessageBuilder {
        MessageBuilder::new()
    }
}

/// Message builder for fluent construction
pub struct MessageBuilder {
    id: Option<String>,
    role: Option<MessageRole>,
    content: Option<MessageContent>,
    metadata: HashMap<String, serde_json::Value>,
    timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new() -> Self {
        Self {
            id: None,
            role: None,
            content: None,
            metadata: HashMap::new(),
            timestamp: None,
        }
    }

    /// Set the message ID
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the message role
    pub fn role(mut self, role: MessageRole) -> Self {
        self.role = Some(role);
        self
    }

    /// Set the message content
    pub fn content(mut self, content: impl Into<MessageContent>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Set the timestamp
    pub fn timestamp(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Build the message
    pub fn build(self) -> Result<Message, MessageBuilderError> {
        let role = self.role.ok_or(MessageBuilderError::MissingRole)?;
        let content = self.content.ok_or(MessageBuilderError::MissingContent)?;

        Ok(Message {
            id: self.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            role,
            content,
            metadata: self.metadata,
            timestamp: self.timestamp.unwrap_or_else(chrono::Utc::now),
        })
    }
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur when building a message
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MessageBuilderError {
    #[error("missing role")]
    MissingRole,

    #[error("missing content")]
    MissingContent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_text_message() {
        let msg = Message::user("hello");
        assert_eq!(msg.role(), &MessageRole::User);
        assert_eq!(msg.content().as_text().unwrap(), "hello");
    }

    #[test]
    fn test_create_system_message() {
        let msg = Message::system("System prompt");
        assert_eq!(msg.role(), &MessageRole::System);
        assert_eq!(msg.content().as_text().unwrap(), "System prompt");
    }

    #[test]
    fn test_create_assistant_message() {
        let msg = Message::assistant("Assistant response");
        assert_eq!(msg.role(), &MessageRole::Assistant);
        assert_eq!(msg.content().as_text().unwrap(), "Assistant response");
    }

    #[test]
    fn test_create_tool_message() {
        let msg = Message::tool("Tool result");
        assert_eq!(msg.role(), &MessageRole::Tool);
        assert_eq!(msg.content().as_text().unwrap(), "Tool result");
    }

    #[test]
    fn test_message_with_parts() {
        let parts = vec![
            ContentPart::text("Hello"),
            ContentPart::thinking("Let me think about this"),
        ];
        let msg = Message::user(parts);
        assert_eq!(msg.role(), &MessageRole::User);
        let content_parts = msg.content().as_parts().unwrap();
        assert_eq!(content_parts.len(), 2);
    }

    #[test]
    fn test_message_with_tool_use() {
        let tool_use =
            ContentPart::tool_use("call_123", "search", serde_json::json!({"query": "test"}));
        let msg = Message::assistant(vec![tool_use]);
        assert_eq!(msg.role(), &MessageRole::Assistant);
        let parts = msg.content().as_parts().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].content_type(), "tool_use");
    }

    #[test]
    fn test_message_with_tool_result() {
        let result =
            ContentPart::tool_result("call_123", "search", ToolOutput::text("Search results"));
        let msg = Message::tool(vec![result]);
        assert_eq!(msg.role(), &MessageRole::Tool);
        let parts = msg.content().as_parts().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].content_type(), "tool_result");
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("test");
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.id(), parsed.id());
        assert_eq!(msg.role(), parsed.role());
        assert_eq!(msg.content(), parsed.content());
    }

    #[test]
    fn test_message_with_metadata() {
        let msg = Message::user("test").with_metadata("key", serde_json::json!("value"));
        assert_eq!(msg.get_metadata("key"), Some(&serde_json::json!("value")));
    }

    #[test]
    fn test_message_builder() {
        let msg = Message::builder()
            .role(MessageRole::User)
            .content("Hello")
            .metadata("key", serde_json::json!("value"))
            .build()
            .unwrap();
        assert_eq!(msg.role(), &MessageRole::User);
        assert_eq!(msg.content().as_text().unwrap(), "Hello");
        assert_eq!(msg.get_metadata("key"), Some(&serde_json::json!("value")));
    }

    #[test]
    fn test_message_builder_missing_role() {
        let result = Message::builder().content("test").build();
        assert_eq!(result, Err(MessageBuilderError::MissingRole));
    }

    #[test]
    fn test_message_builder_missing_content() {
        let result = Message::builder().role(MessageRole::User).build();
        assert_eq!(result, Err(MessageBuilderError::MissingContent));
    }

    #[test]
    fn test_content_part_text() {
        let part = ContentPart::text("Hello world");
        assert_eq!(part.content_type(), "text");
    }

    #[test]
    fn test_content_part_thinking() {
        let part = ContentPart::thinking("Thinking about the problem");
        assert_eq!(part.content_type(), "thinking");
    }

    #[test]
    fn test_message_content_to_parts() {
        let content = MessageContent::text("test");
        let parts = content.to_parts();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].content_type(), "text");
    }

    #[test]
    fn test_message_role_display() {
        assert_eq!(MessageRole::System.to_string(), "system");
        assert_eq!(MessageRole::User.to_string(), "user");
        assert_eq!(MessageRole::Assistant.to_string(), "assistant");
        assert_eq!(MessageRole::Tool.to_string(), "tool");
    }

    #[test]
    fn test_message_id_is_unique() {
        let msg1 = Message::user("test1");
        let msg2 = Message::user("test2");
        assert_ne!(msg1.id(), msg2.id());
    }

    #[test]
    fn test_message_has_timestamp() {
        let msg = Message::user("test");
        let before = chrono::Utc::now() - chrono::Duration::seconds(1);
        let after = chrono::Utc::now() + chrono::Duration::seconds(1);
        assert!(msg.timestamp() > &before);
        assert!(msg.timestamp() < &after);
    }

    #[test]
    fn test_content_part_serde_text() {
        let part = ContentPart::text("test");
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""text":"test""#));
    }

    #[test]
    fn test_content_part_serde_tool_use() {
        let part = ContentPart::tool_use(
            "call_123",
            "function_name",
            serde_json::json!({"arg": "value"}),
        );
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains(r#""type":"tool_use""#));
        assert!(json.contains(r#""id":"call_123""#));
        assert!(json.contains(r#""name":"function_name""#));
    }

    #[test]
    fn test_tool_output_from_string() {
        let output: ToolOutput = "test output".into();
        assert!(matches!(output, ToolOutputVariant::Text(_)));
    }

    #[test]
    fn test_message_content_from_vec() {
        let parts = vec![ContentPart::text("test")];
        let content: MessageContent = parts.into();
        assert!(content.as_parts().is_some());
    }
}
