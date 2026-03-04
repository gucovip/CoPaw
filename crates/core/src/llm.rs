//! LLM provider trait and related types for CoPaw.
//!
//! This module provides the core abstraction for LLM providers that can be used by
//! the CoPaw agent, inspired by AgentScope's chat model system and Python's
//! model factory pattern.
//!
//! # Python Reference
//!
//! - `src/copaw/agents/model_factory.py` - Model factory and chat model creation
//! - `src/copaw/providers/` - Provider definitions and model configurations
//! - AgentScope's `ChatModelBase` and `OpenAIChatModel`

use crate::message::Message;
use serde::{Deserialize, Serialize};

/// Error type for LLM provider failures.
#[derive(Debug, thiserror::Error)]
pub enum LLMError {
    /// LLM API request failed.
    #[error("Request failed: {0}")]
    RequestFailed(String),

    /// Authentication failed (invalid API key, etc.).
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Invalid response from LLM.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Streaming error.
    #[error("Stream error: {0}")]
    StreamError(String),

    /// I/O error during LLM communication.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Token usage information for an LLM response.
///
/// Corresponds to the usage information returned by LLM APIs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: u32,

    /// Number of tokens in the completion.
    pub completion_tokens: u32,

    /// Total tokens (prompt + completion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
}

impl Usage {
    /// Create a new usage record.
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: Some(prompt_tokens + completion_tokens),
        }
    }

    /// Get the total token count.
    pub fn total_tokens(&self) -> u32 {
        self.total_tokens
            .unwrap_or(self.prompt_tokens + self.completion_tokens)
    }
}

/// Tool/function call from an LLM response.
///
/// Corresponds to tool calls in AgentScope's message system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub id: String,

    /// Name of the tool/function to call.
    pub name: String,

    /// Arguments to pass to the tool (JSON object).
    pub arguments: serde_json::Value,
}

impl ToolCall {
    /// Create a new tool call.
    pub fn new<S: Into<String>>(id: S, name: S, arguments: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }

    /// Get the tool call ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the tool name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the tool arguments.
    pub fn arguments(&self) -> &serde_json::Value {
        &self.arguments
    }
}

/// Response from an LLM provider.
///
/// Contains the message content, optional tool calls, and usage information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LLMResponse {
    /// The response message content.
    pub content: Message,

    /// Optional tool calls requested by the LLM.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// Token usage information.
    pub usage: Usage,
}

impl LLMResponse {
    /// Create a new LLM response.
    pub fn new(content: Message, usage: Usage) -> Self {
        Self {
            content,
            tool_calls: None,
            usage,
        }
    }

    /// Create a new LLM response with tool calls.
    pub fn with_tool_calls(content: Message, tool_calls: Vec<ToolCall>, usage: Usage) -> Self {
        Self {
            content,
            tool_calls: Some(tool_calls),
            usage,
        }
    }

    /// Get the response content.
    pub fn content(&self) -> &Message {
        &self.content
    }

    /// Get the tool calls if any.
    pub fn tool_calls(&self) -> Option<&[ToolCall]> {
        self.tool_calls.as_deref()
    }

    /// Get the usage information.
    pub fn usage(&self) -> &Usage {
        &self.usage
    }
}

/// Tool choice setting for LLM requests.
///
/// Controls whether the LLM should use tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoice {
    /// Let the LLM decide whether to use tools.
    #[default]
    Auto,

    /// Do not use tools.
    None,

    /// Require tool use (force the LLM to call a tool).
    Required,
}

impl ToolChoice {
    /// Get the string representation of tool choice.
    pub fn as_str(&self) -> &str {
        match self {
            ToolChoice::Auto => "auto",
            ToolChoice::None => "none",
            ToolChoice::Required => "required",
        }
    }
}

impl std::fmt::Display for ToolChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Function parameter schema for tool definitions.
///
/// Corresponds to JSON Schema for function parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionSchema {
    /// Function name.
    pub name: String,

    /// Function description.
    pub description: String,

    /// JSON Schema for parameters.
    pub parameters: serde_json::Value,
}

impl FunctionSchema {
    /// Create a new function schema.
    pub fn new<S: Into<String>>(name: S, description: S, parameters: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }

    /// Get the function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the function description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get the parameters schema.
    pub fn parameters(&self) -> &serde_json::Value {
        &self.parameters
    }
}

/// Tool schema for LLM function calling.
///
/// Defines a tool that can be called by the LLM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSchema {
    /// Tool type (typically "function").
    #[serde(rename = "type")]
    pub tool_type: String,

    /// Function definition.
    pub function: FunctionSchema,
}

impl ToolSchema {
    /// Create a new tool schema.
    pub fn new(function: FunctionSchema) -> Self {
        Self {
            tool_type: "function".to_string(),
            function,
        }
    }

    /// Create a new tool schema from components.
    pub fn from_parts<S: Into<String>>(
        name: S,
        description: S,
        parameters: serde_json::Value,
    ) -> Self {
        Self::new(FunctionSchema::new(name, description, parameters))
    }

    /// Get the tool type.
    pub fn tool_type(&self) -> &str {
        &self.tool_type
    }

    /// Get the function definition.
    pub fn function(&self) -> &FunctionSchema {
        &self.function
    }
}

/// Core trait that all LLM providers must implement.
///
/// This trait defines the interface for LLM providers that can be used by
/// the CoPaw agent. LLM providers are responsible for making API calls to
/// language models and returning responses.
///
/// # Python Reference
///
/// Corresponds to AgentScope's `ChatModelBase` class.
/// See `src/copaw/agents/model_factory.py` for the Python factory implementation.
///
/// # Example
///
/// ```rust
/// use copaw_core::llm::{
///     LLMProvider, LLMResponse, ToolChoice, ToolSchema, Usage, LLMError,
/// };
/// use copaw_core::message::Message;
///
/// struct MockLLM;
///
/// #[async_trait::async_trait]
/// impl LLMProvider for MockLLM {
///     async fn chat_completion(
///         &self,
///         messages: Vec<Message>,
///         tools: Option<Vec<ToolSchema>>,
///         tool_choice: Option<ToolChoice>,
///     ) -> Result<LLMResponse, LLMError> {
///         // Implementation here
///         # Ok(LLMResponse::new(
///         #     Message::assistant("Hello"),
///         #     Usage::new(10, 5),
///         # ))
///     }
///
///     fn count_tokens(&self, messages: &[Message]) -> usize {
///         // Implementation here
///         # messages.len() * 10
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    /// Perform a chat completion request.
    ///
    /// # Parameters
    ///
    /// - `messages`: The conversation history as a list of messages.
    /// - `tools`: Optional list of tools that the LLM can call.
    /// - `tool_choice`: Optional setting for tool usage behavior.
    ///
    /// # Returns
    ///
    /// An `LLMResponse` containing the LLM's response, any tool calls,
    /// and token usage information.
    ///
    /// # Errors
    ///
    /// Returns an `LLMError` if the request fails.
    ///
    /// # Python Reference
    ///
    /// Corresponds to `ChatModelBase.chat()` in AgentScope.
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolSchema>>,
        tool_choice: Option<ToolChoice>,
    ) -> Result<LLMResponse, LLMError>;

    /// Count tokens in a list of messages.
    ///
    /// This is a synchronous operation that estimates or calculates
    /// the number of tokens in the given messages.
    ///
    /// # Parameters
    ///
    /// - `messages`: The messages to count tokens for.
    ///
    /// # Returns
    ///
    /// The estimated token count.
    ///
    /// # Python Reference
    ///
    /// In Python, this is often handled by tokenizers from the
    /// `tiktoken` or similar libraries.
    fn count_tokens(&self, messages: &[Message]) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLLM;

    #[async_trait::async_trait]
    impl LLMProvider for MockLLM {
        async fn chat_completion(
            &self,
            _messages: Vec<Message>,
            _tools: Option<Vec<ToolSchema>>,
            _tool_choice: Option<ToolChoice>,
        ) -> Result<LLMResponse, LLMError> {
            Ok(LLMResponse::new(
                Message::assistant("Hello"),
                Usage::new(10, 5),
            ))
        }

        fn count_tokens(&self, messages: &[Message]) -> usize {
            messages.len() * 10
        }
    }

    #[tokio::test]
    async fn test_chat_completion() {
        let llm = MockLLM;
        let response = llm
            .chat_completion(vec![Message::user("hi")], None, None)
            .await
            .unwrap();
        assert_eq!(response.content().content().as_text().unwrap(), "Hello");
    }

    #[tokio::test]
    async fn test_chat_completion_with_tools() {
        let llm = MockLLM;
        let tools = vec![ToolSchema::from_parts(
            "test_tool",
            "A test tool",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            }),
        )];
        let response = llm
            .chat_completion(vec![Message::user("hi")], Some(tools), None)
            .await
            .unwrap();
        assert_eq!(response.content().content().as_text().unwrap(), "Hello");
    }

    #[tokio::test]
    async fn test_chat_completion_with_tool_choice() {
        let llm = MockLLM;
        let response = llm
            .chat_completion(vec![Message::user("hi")], None, Some(ToolChoice::Required))
            .await
            .unwrap();
        assert_eq!(response.content().content().as_text().unwrap(), "Hello");
    }

    #[test]
    fn test_count_tokens() {
        let llm = MockLLM;
        let messages = vec![
            Message::user("Hello"),
            Message::assistant("Hi there"),
            Message::user("How are you?"),
        ];
        assert_eq!(llm.count_tokens(&messages), 30);
    }

    #[test]
    fn test_usage_new() {
        let usage = Usage::new(10, 5);
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens(), 15);
    }

    #[test]
    fn test_usage_total_tokens() {
        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: Some(20),
        };
        assert_eq!(usage.total_tokens(), 20);
    }

    #[test]
    fn test_tool_call_new() {
        let tool_call = ToolCall::new("call_123", "search", serde_json::json!({"query": "test"}));
        assert_eq!(tool_call.id(), "call_123");
        assert_eq!(tool_call.name(), "search");
        assert_eq!(tool_call.arguments(), &serde_json::json!({"query": "test"}));
    }

    #[test]
    fn test_llm_response_new() {
        let content = Message::assistant("Response");
        let usage = Usage::new(10, 5);
        let response = LLMResponse::new(content.clone(), usage.clone());
        assert_eq!(response.content(), &content);
        assert_eq!(response.tool_calls(), None);
        assert_eq!(response.usage(), &usage);
    }

    #[test]
    fn test_llm_response_with_tool_calls() {
        let content = Message::assistant("Response");
        let tool_calls = vec![ToolCall::new(
            "call_123",
            "search",
            serde_json::json!({"query": "test"}),
        )];
        let usage = Usage::new(10, 5);
        let response = LLMResponse::with_tool_calls(content.clone(), tool_calls, usage);
        assert!(response.tool_calls().is_some());
        assert_eq!(response.tool_calls().unwrap().len(), 1);
    }

    #[test]
    fn test_tool_choice_display() {
        assert_eq!(ToolChoice::Auto.to_string(), "auto");
        assert_eq!(ToolChoice::None.to_string(), "none");
        assert_eq!(ToolChoice::Required.to_string(), "required");
    }

    #[test]
    fn test_tool_choice_default() {
        assert_eq!(ToolChoice::default(), ToolChoice::Auto);
    }

    #[test]
    fn test_function_schema_new() {
        let schema = FunctionSchema::new(
            "test_func",
            "A test function",
            serde_json::json!({"type": "object"}),
        );
        assert_eq!(schema.name(), "test_func");
        assert_eq!(schema.description(), "A test function");
        assert_eq!(schema.parameters(), &serde_json::json!({"type": "object"}));
    }

    #[test]
    fn test_tool_schema_new() {
        let function = FunctionSchema::new(
            "test_func",
            "A test function",
            serde_json::json!({"type": "object"}),
        );
        let schema = ToolSchema::new(function);
        assert_eq!(schema.tool_type(), "function");
        assert_eq!(schema.function().name(), "test_func");
    }

    #[test]
    fn test_tool_schema_from_parts() {
        let schema = ToolSchema::from_parts(
            "test_func",
            "A test function",
            serde_json::json!({"type": "object"}),
        );
        assert_eq!(schema.tool_type(), "function");
        assert_eq!(schema.function().name(), "test_func");
    }

    #[test]
    fn test_llm_error_display() {
        let err = LLMError::RequestFailed("test error".to_string());
        assert!(err.to_string().contains("Request failed"));
        assert!(err.to_string().contains("test error"));

        let err = LLMError::AuthFailed("auth error".to_string());
        assert!(err.to_string().contains("Authentication failed"));

        let err = LLMError::RateLimit("rate limit".to_string());
        assert!(err.to_string().contains("Rate limit exceeded"));

        let err = LLMError::InvalidResponse("invalid".to_string());
        assert!(err.to_string().contains("Invalid response"));

        let err = LLMError::StreamError("stream error".to_string());
        assert!(err.to_string().contains("Stream error"));
    }

    #[test]
    fn test_usage_serialization() {
        let usage = Usage::new(10, 5);
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains(r#""prompt_tokens":10"#));
        assert!(json.contains(r#""completion_tokens":5"#));
        assert!(json.contains(r#""total_tokens":15"#));
    }

    #[test]
    fn test_tool_call_serialization() {
        let tool_call = ToolCall::new("call_123", "search", serde_json::json!({"query": "test"}));
        let json = serde_json::to_string(&tool_call).unwrap();
        assert!(json.contains(r#""id":"call_123""#));
        assert!(json.contains(r#""name":"search""#));
        assert!(json.contains(r#""query":"test""#));
    }

    #[test]
    fn test_llm_response_serialization() {
        let content = Message::assistant("Response");
        let usage = Usage::new(10, 5);
        let response = LLMResponse::new(content, usage);
        let json = serde_json::to_string(&response).unwrap();
        // Verify it can be serialized
        assert!(json.len() > 0);
    }

    #[test]
    fn test_tool_schema_serialization() {
        let schema = ToolSchema::from_parts(
            "test_func",
            "A test function",
            serde_json::json!({"type": "object"}),
        );
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains(r#""type":"function""#));
        assert!(json.contains(r#""name":"test_func""#));
        assert!(json.contains(r#""description":"A test function""#));
    }

    #[tokio::test]
    async fn test_chat_completion_empty_messages() {
        let llm = MockLLM;
        let response = llm.chat_completion(vec![], None, None).await.unwrap();
        assert_eq!(response.content().content().as_text().unwrap(), "Hello");
    }

    #[test]
    fn test_count_tokens_empty() {
        let llm = MockLLM;
        let messages: Vec<Message> = vec![];
        assert_eq!(llm.count_tokens(&messages), 0);
    }
}
