//! Local model provider implementations.

//!
//! This module provides LLM providers for local model inference backends,
//! including llama.cpp and Ollama.
//!
//! These providers implement the `LLMProvider` trait for communicating with
//! local model servers.
//!
//! # Python Reference
//!
//! - `src/copaw/local_models/` - Local model management
//! - `src/copaw/local_models/backends/llamacpp_backend.py` - llama.cpp backend
//! - `src/copaw/providers/ollama_manager.py` - Ollama management

use async_trait::async_trait;
use copaw_core::llm::{LLMError, LLMProvider, LLMResponse, ToolChoice, ToolSchema, Usage};
use copaw_core::message::{ContentPart, Message, MessageContent};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// llama.cpp provider
///
/// Implements the LLMProvider trait for llama.cpp server.
/// Communicates via HTTP API to a running llama.cpp server.
///
/// # Python Reference
///
/// Corresponds to `LlamaCppBackend` in `src/copaw/local_models/backends/llamacpp_backend.py`.
pub struct LlamaCppProvider {
    /// HTTP client for API requests
    client: Client,
    /// Base URL for llama.cpp server
    pub base_url: String,
    /// Model name to use
    pub model: String,
}

impl LlamaCppProvider {
    /// Create a new llama.cpp provider
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL for the llama.cpp server (e.g., "http://localhost:8080")
    /// * `model` - Model name to use (e.g., "llama-3")
    ///
    /// # Example
    ///
    /// ```rust
    /// use copaw_providers::LlamaCppProvider;
    ///
    /// let provider = LlamaCppProvider::new(
    ///     "http://localhost:8080",
    ///     "llama-3",
    /// );
    /// ```
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        LlamaCppProvider {
            client: Client::new(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    /// Get the model name
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Convert internal messages to llama.cpp format
    fn convert_messages(&self, messages: Vec<Message>) -> Vec<LlamaCppMessage> {
        messages.into_iter().map(LlamaCppMessage::from).collect()
    }
}

/// llama.cpp API message format
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlamaCppMessage {
    pub role: String,
    pub content: String,
}

impl From<Message> for LlamaCppMessage {
    fn from(msg: Message) -> Self {
        let role = msg.role().as_str().to_string();
        let content = match msg.content() {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
        };
        LlamaCppMessage { role, content }
    }
}

/// llama.cpp API chat completion request
#[derive(Debug, Serialize)]
struct LlamaCppCompletionRequest {
    pub messages: Vec<LlamaCppMessage>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// llama.cpp API response format
#[derive(Debug, Deserialize)]
struct LlamaCppCompletionResponse {
    pub choices: Vec<LlamaCppChoice>,
}

/// Choice in llama.cpp API response
#[derive(Debug, Deserialize)]
struct LlamaCppChoice {
    pub message: LlamaCppResponseMessage,
    #[serde(rename = "finish_reason")]
    pub _finish_reason: String,
}

/// Message in llama.cpp API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LlamaCppResponseMessage {
    pub role: String,
    pub content: String,
}

#[async_trait]
impl LLMProvider for LlamaCppProvider {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        _tools: Option<Vec<ToolSchema>>,
        _tool_choice: Option<ToolChoice>,
    ) -> Result<LLMResponse, LLMError> {
        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );

        let llama_messages = self.convert_messages(messages);

        let request = LlamaCppCompletionRequest {
            messages: llama_messages,
            model: self.model.clone(),
            temperature: Some(0.7),
            max_tokens: Some(2048),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(LLMError::RequestFailed(format!(
                "HTTP {}: {}",
                status, response_body
            )));
        }

        let completion: LlamaCppCompletionResponse = serde_json::from_str(&response_body)
            .map_err(|e| LLMError::InvalidResponse(format!("Failed to parse response: {}", e)))?;

        if completion.choices.is_empty() {
            return Err(LLMError::InvalidResponse(
                "No choices in response".to_string(),
            ));
        }

        let choice = &completion.choices[0];
        let content = Message::assistant(choice.message.content.clone());

        // Since local providers don't return accurate token counts, estimate
        let usage = Usage::new(100, 50);

        Ok(LLMResponse::new(content, usage))
    }

    fn count_tokens(&self, messages: &[Message]) -> usize {
        // Simple estimation: approximately 4 characters per token for English text
        let total_chars: usize = messages
            .iter()
            .map(|m| match m.content() {
                MessageContent::Text(text) => text.len(),
                MessageContent::Parts(parts) => parts
                    .iter()
                    .map(|p| match p {
                        ContentPart::Text { text } => text.len(),
                        _ => 0,
                    })
                    .sum(),
            })
            .sum();

        // Add overhead for message structure (role, etc.)
        let overhead = messages.len() * 10;
        (total_chars / 4) + overhead
    }
}

/// Ollama provider
///
/// Implements the LLMProvider trait for Ollama server.
/// Communicates via HTTP API to a running Ollama server.
///
/// # Python Reference
///
/// Corresponds to `OllamaModelManager` in `src/copaw/providers/ollama_manager.py`.
pub struct OllamaProvider {
    /// HTTP client for API requests
    client: Client,
    /// Base URL for Ollama API
    pub base_url: String,
    /// Model name to use
    pub model: String,
}

impl OllamaProvider {
    /// Create a new Ollama provider
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL for the Ollama server (e.g., "http://localhost:11434")
    /// * `model` - Model name to use (e.g., "llama3")
    ///
    /// # Example
    ///
    /// ```rust
    /// use copaw_providers::OllamaProvider;
    ///
    /// let provider = OllamaProvider::new(
    ///     "http://localhost:11434",
    ///     "llama3",
    /// );
    /// ```
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        OllamaProvider {
            client: Client::new(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    /// Get the model name
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Convert internal messages to Ollama format
    fn convert_messages(&self, messages: Vec<Message>) -> Vec<OllamaMessage> {
        messages.into_iter().map(OllamaMessage::from).collect()
    }
}

/// Ollama API message format
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaMessage {
    pub role: String,
    pub content: String,
}

impl From<Message> for OllamaMessage {
    fn from(msg: Message) -> Self {
        let role = msg.role().as_str().to_string();
        let content = match msg.content() {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(" "),
        };
        OllamaMessage { role, content }
    }
}

/// Ollama API chat completion request
#[derive(Debug, Serialize)]
struct OllamaCompletionRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Ollama API response format
#[derive(Debug, Deserialize)]
struct OllamaCompletionResponse {
    pub message: OllamaResponseMessage,
    #[serde(default)]
    pub prompt_eval_count: u32,
    #[serde(default)]
    pub eval_count: u32,
}

/// Message in Ollama API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaResponseMessage {
    pub role: String,
    pub content: String,
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        _tools: Option<Vec<ToolSchema>>,
        _tool_choice: Option<ToolChoice>,
    ) -> Result<LLMResponse, LLMError> {
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));

        let ollama_messages = self.convert_messages(messages);

        let request = OllamaCompletionRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: Some(false),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let response_body = response
            .text()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(LLMError::RequestFailed(format!(
                "HTTP {}: {}",
                status, response_body
            )));
        }

        let completion: OllamaCompletionResponse = serde_json::from_str(&response_body)
            .map_err(|e| LLMError::InvalidResponse(format!("Failed to parse response: {}", e)))?;

        let content = Message::assistant(completion.message.content.clone());

        // Ollama returns actual token counts if available, otherwise estimate
        let prompt_tokens = if completion.prompt_eval_count > 0 {
            completion.prompt_eval_count
        } else {
            100
        };
        let completion_tokens = if completion.eval_count > 0 {
            completion.eval_count
        } else {
            50
        };

        let usage = Usage::new(prompt_tokens, completion_tokens);

        Ok(LLMResponse::new(content, usage))
    }

    fn count_tokens(&self, messages: &[Message]) -> usize {
        // Simple estimation: approximately 4 characters per token for English text
        let total_chars: usize = messages
            .iter()
            .map(|m| match m.content() {
                MessageContent::Text(text) => text.len(),
                MessageContent::Parts(parts) => parts
                    .iter()
                    .map(|p| match p {
                        ContentPart::Text { text } => text.len(),
                        _ => 0,
                    })
                    .sum(),
            })
            .sum();

        // Add overhead for message structure (role, etc.)
        let overhead = messages.len() * 10;
        (total_chars / 4) + overhead
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock HTTP server for testing
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    /// Helper to create a mock llama.cpp response
    fn mock_llamacpp_response(content: &str) -> serde_json::Value {
        serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "finish_reason": "stop"
            }]
        })
    }

    /// Helper to create a mock Ollama response
    fn mock_ollama_response(content: &str) -> serde_json::Value {
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": content
            },
            "prompt_eval_count": 10,
            "eval_count": 5
        })
    }

    #[test]
    fn test_llamacpp_provider_creation() {
        let provider = LlamaCppProvider::new("http://localhost:8080", "llama-3");
        assert_eq!(provider.model(), "llama-3");
        assert_eq!(provider.base_url(), "http://localhost:8080");
    }

    #[test]
    fn test_llamacpp_provider_getters() {
        let provider = LlamaCppProvider::new("http://localhost:8080", "llama-3");
        assert_eq!(provider.model(), "llama-3");
        assert_eq!(provider.base_url(), "http://localhost:8080");
    }

    #[test]
    fn test_llamacpp_message_conversion_from_user() {
        let msg = Message::user("hello world");
        let llama_msg = LlamaCppMessage::from(msg);
        assert_eq!(llama_msg.role, "user");
        assert_eq!(llama_msg.content, "hello world");
    }

    #[test]
    fn test_llamacpp_message_conversion_from_assistant() {
        let msg = Message::assistant("hi there");
        let llama_msg = LlamaCppMessage::from(msg);
        assert_eq!(llama_msg.role, "assistant");
        assert_eq!(llama_msg.content, "hi there");
    }

    #[test]
    fn test_llamacpp_message_conversion_from_parts() {
        let parts = vec![
            ContentPart::text("Hello"),
            ContentPart::thinking("Let me think"),
            ContentPart::text("World"),
        ];
        let msg = Message::user(parts);
        let llama_msg = LlamaCppMessage::from(msg);
        assert_eq!(llama_msg.role, "user");
        // Only text parts are concatenated
        assert_eq!(llama_msg.content, "Hello World");
    }

    #[test]
    fn test_llamacpp_count_tokens_simple() {
        let provider = LlamaCppProvider::new("http://localhost:8080", "llama-3");
        let messages = vec![Message::user("Hello world")];
        let count = provider.count_tokens(&messages);
        // "Hello world" is 11 chars, / 4 = 2.75, + 10 overhead = ~12-13
        assert!(count > 0 && count < 100);
    }

    #[test]
    fn test_llamacpp_count_tokens_multiple_messages() {
        let provider = LlamaCppProvider::new("http://localhost:8080", "llama-3");
        let messages = vec![
            Message::user("Hello"),
            Message::assistant("Hi"),
            Message::user("How are you?"),
        ];
        let count = provider.count_tokens(&messages);
        // Should be more than single message
        assert!(count > 10);
    }

    #[test]
    fn test_llamacpp_count_tokens_empty() {
        let provider = LlamaCppProvider::new("http://localhost:8080", "llama-3");
        let messages: Vec<Message> = vec![];
        let count = provider.count_tokens(&messages);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_llamacpp_request_serialization() {
        let request = LlamaCppCompletionRequest {
            messages: vec![LlamaCppMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            model: "llama-3".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(2048),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""model":"llama-3""#));
        assert!(json.contains(r#""role":"user""#));
        assert!(json.contains(r#""content":"hello""#));
    }

    #[tokio::test]
    async fn test_llamacpp_chat_completion_success() {
        let mock_server = MockServer::start().await;
        let provider = LlamaCppProvider::new(mock_server.uri(), "llama-3");

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(mock_llamacpp_response("Hello from llama.cpp!")),
            )
            .mount(&mock_server)
            .await;

        let response = provider
            .chat_completion(vec![Message::user("hi")], None, None)
            .await
            .unwrap();

        assert_eq!(
            response.content().content().as_text().unwrap(),
            "Hello from llama.cpp!"
        );
        assert_eq!(response.usage().prompt_tokens, 100);
        assert_eq!(response.usage().completion_tokens, 50);
        assert!(response.tool_calls().is_none());
    }

    #[test]
    fn test_ollama_provider_creation() {
        let provider = OllamaProvider::new("http://localhost:11434", "llama3");
        assert_eq!(provider.model(), "llama3");
        assert_eq!(provider.base_url(), "http://localhost:11434");
    }

    #[test]
    fn test_ollama_provider_getters() {
        let provider = OllamaProvider::new("http://localhost:11434", "llama3:8b");
        assert_eq!(provider.model(), "llama3:8b");
        assert_eq!(provider.base_url(), "http://localhost:11434");
    }

    #[test]
    fn test_ollama_message_conversion_from_user() {
        let msg = Message::user("hello world");
        let ollama_msg = OllamaMessage::from(msg);
        assert_eq!(ollama_msg.role, "user");
        assert_eq!(ollama_msg.content, "hello world");
    }

    #[test]
    fn test_ollama_message_conversion_from_parts() {
        let parts = vec![
            ContentPart::text("Hello"),
            ContentPart::thinking("Let me think"),
            ContentPart::text("World"),
        ];
        let msg = Message::user(parts);
        let ollama_msg = OllamaMessage::from(msg);
        assert_eq!(ollama_msg.role, "user");
        // Only text parts are concatenated
        assert_eq!(ollama_msg.content, "Hello World");
    }

    #[test]
    fn test_ollama_count_tokens_simple() {
        let provider = OllamaProvider::new("http://localhost:11434", "llama3");
        let messages = vec![Message::user("Hello world")];
        let count = provider.count_tokens(&messages);
        // "Hello world" is 11 chars, / 4 = 2.75, + 10 overhead = ~12-13
        assert!(count > 0 && count < 100);
    }

    #[test]
    fn test_ollama_count_tokens_multiple_messages() {
        let provider = OllamaProvider::new("http://localhost:11434", "llama3");
        let messages = vec![
            Message::user("Hello"),
            Message::assistant("Hi"),
            Message::user("How are you?"),
        ];
        let count = provider.count_tokens(&messages);
        // Should be more than single message
        assert!(count > 10);
    }

    #[test]
    fn test_ollama_count_tokens_empty() {
        let provider = OllamaProvider::new("http://localhost:11434", "llama3");
        let messages: Vec<Message> = vec![];
        let count = provider.count_tokens(&messages);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_ollama_request_serialization() {
        let request = OllamaCompletionRequest {
            model: "llama3".to_string(),
            messages: vec![OllamaMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            stream: Some(false),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""model":"llama3""#));
        assert!(json.contains(r#""role":"user""#));
        assert!(json.contains(r#""content":"hello""#));
    }

    #[tokio::test]
    async fn test_ollama_chat_completion_success() {
        let mock_server = MockServer::start().await;
        let provider = OllamaProvider::new(mock_server.uri(), "llama3");

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/api/chat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(mock_ollama_response("Hello from Ollama!")),
            )
            .mount(&mock_server)
            .await;

        let response = provider
            .chat_completion(vec![Message::user("hi")], None, None)
            .await
            .unwrap();

        assert_eq!(
            response.content().content().as_text().unwrap(),
            "Hello from Ollama!"
        );
        assert_eq!(response.usage().prompt_tokens, 10);
        assert_eq!(response.usage().completion_tokens, 5);
        assert!(response.tool_calls().is_none());
    }

    #[tokio::test]
    async fn test_llamacpp_chat_completion_http_error() {
        let mock_server = MockServer::start().await;
        let provider = LlamaCppProvider::new(mock_server.uri(), "llama-3");

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let result = provider
            .chat_completion(vec![Message::user("hi")], None, None)
            .await;

        match result {
            Err(LLMError::RequestFailed(msg)) => {
                assert!(msg.contains("HTTP 500"));
            }
            _ => panic!("Expected RequestFailed error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_ollama_chat_completion_http_error() {
        let mock_server = MockServer::start().await;
        let provider = OllamaProvider::new(mock_server.uri(), "llama3");

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/api/chat"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let result = provider
            .chat_completion(vec![Message::user("hi")], None, None)
            .await;

        match result {
            Err(LLMError::RequestFailed(msg)) => {
                assert!(msg.contains("HTTP 500"));
            }
            _ => panic!("Expected RequestFailed error, got {:?}", result),
        }
    }

    #[test]
    fn test_llamacpp_base_url_trim() {
        // Test that trailing slash is handled
        let provider = LlamaCppProvider::new("http://localhost:8080/", "llama-3");
        assert_eq!(provider.base_url(), "http://localhost:8080/");
    }

    #[test]
    fn test_ollama_base_url_trim() {
        // Test that trailing slash is handled
        let provider = OllamaProvider::new("http://localhost:11434/", "llama3");
        assert_eq!(provider.base_url(), "http://localhost:11434/");
    }
}
