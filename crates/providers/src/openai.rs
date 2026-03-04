//! OpenAI provider implementation

use async_trait::async_trait;
use copaw_core::llm::{LLMError, LLMProvider, LLMResponse, ToolChoice, ToolSchema, Usage};
use copaw_core::message::{ContentPart, Message, MessageContent};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// OpenAI API message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: String,
}

impl From<Message> for OpenAIMessage {
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
        OpenAIMessage { role, content }
    }
}

/// OpenAI API chat completion request
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoiceValue>,
}

/// Tool choice value for OpenAI API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ToolChoiceValue {
    Auto,
    None,
    Required,
}

impl From<ToolChoice> for ToolChoiceValue {
    fn from(choice: ToolChoice) -> Self {
        match choice {
            ToolChoice::Auto => ToolChoiceValue::Auto,
            ToolChoice::None => ToolChoiceValue::None,
            ToolChoice::Required => ToolChoiceValue::Required,
        }
    }
}

/// OpenAI API tool format
#[derive(Debug, Serialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAIFunction,
}

impl From<ToolSchema> for OpenAITool {
    fn from(schema: ToolSchema) -> Self {
        OpenAITool {
            tool_type: schema.tool_type().to_string(),
            function: OpenAIFunction {
                name: schema.function().name().to_string(),
                description: schema.function().description().to_string(),
                parameters: schema.function().parameters().clone(),
            },
        }
    }
}

/// OpenAI API function format
#[derive(Debug, Serialize)]
struct OpenAIFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// OpenAI API response format
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[serde(rename = "id")]
    pub _id: String,
    #[serde(rename = "object")]
    pub _object: String,
    #[serde(rename = "created")]
    pub _created: u64,
    #[serde(rename = "model")]
    pub _model: String,
    pub choices: Vec<Choice>,
    pub usage: ResponseUsage,
}

/// Choice in OpenAI API response
#[derive(Debug, Deserialize)]
struct Choice {
    #[serde(rename = "index")]
    pub _index: u32,
    pub message: ResponseMessage,
    #[serde(rename = "finish_reason")]
    pub _finish_reason: String,
}

/// Message in OpenAI API response
#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[serde(rename = "role")]
    pub _role: String,
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ResponseToolCall>,
}

/// Tool call in OpenAI API response
#[derive(Debug, Deserialize)]
struct ResponseToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub _tool_type: String,
    pub function: ResponseFunction,
}

/// Function in OpenAI API tool call
#[derive(Debug, Deserialize)]
struct ResponseFunction {
    pub name: String,
    pub arguments: String,
}

/// Usage in OpenAI API response
#[derive(Debug, Deserialize)]
struct ResponseUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    #[serde(rename = "total_tokens")]
    pub _total_tokens: u32,
}

/// OpenAI error response format
#[derive(Debug, Deserialize)]
struct OpenAIErrorResponse {
    pub error: OpenAIError,
}

#[derive(Debug, Deserialize)]
struct OpenAIError {
    pub message: String,
    #[serde(rename = "type")]
    pub _error_type: Option<String>,
    #[serde(rename = "code")]
    pub _code: Option<String>,
}

/// OpenAI provider
///
/// Implements the LLMProvider trait for OpenAI-compatible APIs.
/// This includes OpenAI itself as well as compatible services like
/// Azure OpenAI, ModelScope, DashScope, etc.
pub struct OpenAIProvider {
    /// HTTP client for API requests
    client: Client,
    /// Base URL for OpenAI API
    pub base_url: String,
    /// API key for authentication
    pub api_key: String,
    /// Model name to use
    pub model: String,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL for the OpenAI API (e.g., "https://api.openai.com/v1")
    /// * `api_key` - API key for authentication
    /// * `model` - Model name to use (e.g., "gpt-4")
    ///
    /// # Example
    ///
    /// ```rust
    /// use copaw_providers::OpenAIProvider;
    ///
    /// let provider = OpenAIProvider::new(
    ///     "https://api.openai.com/v1",
    ///     "sk-test-key",
    ///     "gpt-4",
    /// );
    /// ```
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        OpenAIProvider {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
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

    /// Get the API key
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Convert internal messages to OpenAI format
    fn convert_messages(&self, messages: Vec<Message>) -> Vec<OpenAIMessage> {
        messages.into_iter().map(OpenAIMessage::from).collect()
    }

    /// Convert tools to OpenAI format
    fn convert_tools(&self, tools: Option<Vec<ToolSchema>>) -> Option<Vec<OpenAITool>> {
        tools.map(|t| t.into_iter().map(OpenAITool::from).collect())
    }

    /// Convert tool choice to OpenAI format
    fn convert_tool_choice(&self, tool_choice: Option<ToolChoice>) -> Option<ToolChoiceValue> {
        tool_choice.map(ToolChoiceValue::from)
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolSchema>>,
        tool_choice: Option<ToolChoice>,
    ) -> Result<LLMResponse, LLMError> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let openai_messages = self.convert_messages(messages);
        let openai_tools = self.convert_tools(tools);
        let openai_tool_choice = self.convert_tool_choice(tool_choice);

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: openai_messages,
            tools: openai_tools,
            tool_choice: openai_tool_choice,
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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
            // Try to parse error response
            if let Ok(error_resp) = serde_json::from_str::<OpenAIErrorResponse>(&response_body) {
                let error_msg = error_resp.error.message;
                return match status.as_u16() {
                    401 => Err(LLMError::AuthFailed(error_msg)),
                    429 => Err(LLMError::RateLimit(error_msg)),
                    _ => Err(LLMError::RequestFailed(format!(
                        "{}: {}",
                        status, error_msg
                    ))),
                };
            }
            return Err(LLMError::RequestFailed(format!(
                "HTTP {}: {}",
                status, response_body
            )));
        }

        let completion: ChatCompletionResponse = serde_json::from_str(&response_body)
            .map_err(|e| LLMError::InvalidResponse(format!("Failed to parse response: {}", e)))?;

        if completion.choices.is_empty() {
            return Err(LLMError::InvalidResponse(
                "No choices in response".to_string(),
            ));
        }

        let choice = &completion.choices[0];
        let content = Message::assistant(choice.message.content.clone().unwrap_or_default());

        let tool_calls = if choice.message.tool_calls.is_empty() {
            None
        } else {
            Some(
                choice
                    .message
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        copaw_core::llm::ToolCall::new(
                            tc.id.clone(),
                            tc.function.name.clone(),
                            serde_json::from_str(&tc.function.arguments).unwrap_or_else(|_| {
                                serde_json::Value::String(tc.function.arguments.clone())
                            }),
                        )
                    })
                    .collect(),
            )
        };

        let usage = Usage::new(
            completion.usage.prompt_tokens,
            completion.usage.completion_tokens,
        );

        if let Some(calls) = tool_calls {
            Ok(LLMResponse::with_tool_calls(content, calls, usage))
        } else {
            Ok(LLMResponse::new(content, usage))
        }
    }

    fn count_tokens(&self, messages: &[Message]) -> usize {
        // Simple estimation: approximately 4 characters per token for English text
        // This is a rough estimate; in production, use a proper tokenizer like tiktoken
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

    /// Helper to create a mock OpenAI response
    fn mock_chat_completion_response(content: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        })
    }

    /// Helper to create a mock OpenAI response with tool calls
    fn mock_chat_completion_with_tools(
        tool_name: &str,
        tool_args: serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "gpt-4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_test123",
                        "type": "function",
                        "function": {
                            "name": tool_name,
                            "arguments": serde_json::to_string(&tool_args).unwrap()
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 20,
                "completion_tokens": 10,
                "total_tokens": 30
            }
        })
    }

    #[test]
    fn test_openai_provider_creation() {
        let provider = OpenAIProvider::new("https://api.openai.com/v1", "sk-test", "gpt-4");
        assert_eq!(provider.model(), "gpt-4");
        assert_eq!(provider.base_url(), "https://api.openai.com/v1");
        assert_eq!(provider.api_key(), "sk-test");
    }

    #[test]
    fn test_openai_provider_getters() {
        let provider = OpenAIProvider::new(
            "https://api.openai.com/v1",
            "sk-test-key-12345",
            "gpt-4-turbo",
        );
        assert_eq!(provider.model(), "gpt-4-turbo");
        assert_eq!(provider.base_url(), "https://api.openai.com/v1");
        assert_eq!(provider.api_key(), "sk-test-key-12345");
    }

    #[test]
    fn test_message_conversion_from_user() {
        let msg = Message::user("hello world");
        let openai_msg = OpenAIMessage::from(msg);
        assert_eq!(openai_msg.role, "user");
        assert_eq!(openai_msg.content, "hello world");
    }

    #[test]
    fn test_message_conversion_from_assistant() {
        let msg = Message::assistant("hi there");
        let openai_msg = OpenAIMessage::from(msg);
        assert_eq!(openai_msg.role, "assistant");
        assert_eq!(openai_msg.content, "hi there");
    }

    #[test]
    fn test_message_conversion_from_system() {
        let msg = Message::system("You are a helpful assistant");
        let openai_msg = OpenAIMessage::from(msg);
        assert_eq!(openai_msg.role, "system");
        assert_eq!(openai_msg.content, "You are a helpful assistant");
    }

    #[test]
    fn test_message_conversion_from_parts() {
        let parts = vec![
            ContentPart::text("Hello"),
            ContentPart::thinking("Let me think"),
            ContentPart::text("World"),
        ];
        let msg = Message::user(parts);
        let openai_msg = OpenAIMessage::from(msg);
        assert_eq!(openai_msg.role, "user");
        // Only text parts are concatenated
        assert_eq!(openai_msg.content, "Hello World");
    }

    #[test]
    fn test_tool_choice_conversion() {
        let auto_value: ToolChoiceValue = ToolChoice::Auto.into();
        match auto_value {
            ToolChoiceValue::Auto => {}
            _ => panic!("Expected Auto"),
        }

        let none_value: ToolChoiceValue = ToolChoice::None.into();
        match none_value {
            ToolChoiceValue::None => {}
            _ => panic!("Expected None"),
        }

        let required_value: ToolChoiceValue = ToolChoice::Required.into();
        match required_value {
            ToolChoiceValue::Required => {}
            _ => panic!("Expected Required"),
        }
    }

    #[test]
    fn test_tool_schema_conversion() {
        let schema = ToolSchema::from_parts(
            "test_function",
            "A test function",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            }),
        );

        let openai_tool: OpenAITool = schema.into();
        assert_eq!(openai_tool.tool_type, "function");
        assert_eq!(openai_tool.function.name, "test_function");
        assert_eq!(openai_tool.function.description, "A test function");
    }

    #[test]
    fn test_count_tokens_simple() {
        let provider = OpenAIProvider::new("https://api.openai.com/v1", "sk-test", "gpt-4");
        let messages = vec![Message::user("Hello world")];
        let count = provider.count_tokens(&messages);
        // "Hello world" is 11 chars, / 4 = 2.75, + 10 overhead = ~12-13
        assert!(count > 0 && count < 100);
    }

    #[test]
    fn test_count_tokens_multiple_messages() {
        let provider = OpenAIProvider::new("https://api.openai.com/v1", "sk-test", "gpt-4");
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
    fn test_count_tokens_empty() {
        let provider = OpenAIProvider::new("https://api.openai.com/v1", "sk-test", "gpt-4");
        let messages: Vec<Message> = vec![];
        let count = provider.count_tokens(&messages);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_chat_completion_success() {
        let mock_server = MockServer::start().await;
        let provider = OpenAIProvider::new(mock_server.uri(), "sk-test", "gpt-4");

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(mock_chat_completion_response("Hello from mock!")),
            )
            .mount(&mock_server)
            .await;

        let response = provider
            .chat_completion(vec![Message::user("hi")], None, None)
            .await
            .unwrap();

        assert_eq!(
            response.content().content().as_text().unwrap(),
            "Hello from mock!"
        );
        assert_eq!(response.usage().prompt_tokens, 10);
        assert_eq!(response.usage().completion_tokens, 5);
        assert!(response.tool_calls().is_none());
    }

    #[tokio::test]
    async fn test_chat_completion_with_tools() {
        let mock_server = MockServer::start().await;
        let provider = OpenAIProvider::new(mock_server.uri(), "sk-test", "gpt-4");

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                mock_chat_completion_with_tools("search", serde_json::json!({"query": "test"})),
            ))
            .mount(&mock_server)
            .await;

        let tools = vec![ToolSchema::from_parts(
            "search",
            "Search the web",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                }
            }),
        )];

        let response = provider
            .chat_completion(vec![Message::user("search for cats")], Some(tools), None)
            .await
            .unwrap();

        assert!(response.tool_calls().is_some());
        let calls = response.tool_calls().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name(), "search");
        assert_eq!(calls[0].arguments(), &serde_json::json!({"query": "test"}));
    }

    #[tokio::test]
    async fn test_chat_completion_auth_error() {
        let mock_server = MockServer::start().await;
        let provider = OpenAIProvider::new(mock_server.uri(), "sk-invalid", "gpt-4");

        let error_response = serde_json::json!({
            "error": {
                "message": "Invalid API key",
                "type": "invalid_request_error",
                "code": "invalid_api_key"
            }
        });

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_json(error_response))
            .mount(&mock_server)
            .await;

        let result = provider
            .chat_completion(vec![Message::user("hi")], None, None)
            .await;

        match result {
            Err(LLMError::AuthFailed(msg)) => {
                assert!(msg.contains("Invalid API key"));
            }
            _ => panic!("Expected AuthFailed error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_chat_completion_rate_limit() {
        let mock_server = MockServer::start().await;
        let provider = OpenAIProvider::new(mock_server.uri(), "sk-test", "gpt-4");

        let error_response = serde_json::json!({
            "error": {
                "message": "Rate limit exceeded",
                "type": "rate_limit_error"
            }
        });

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/chat/completions"))
            .respond_with(ResponseTemplate::new(429).set_body_json(error_response))
            .mount(&mock_server)
            .await;

        let result = provider
            .chat_completion(vec![Message::user("hi")], None, None)
            .await;

        match result {
            Err(LLMError::RateLimit(msg)) => {
                assert!(msg.contains("Rate limit exceeded"));
            }
            _ => panic!("Expected RateLimit error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_chat_completion_with_tool_choice_required() {
        let mock_server = MockServer::start().await;
        let provider = OpenAIProvider::new(mock_server.uri(), "sk-test", "gpt-4");

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                mock_chat_completion_with_tools("get_weather", serde_json::json!({"city": "SF"})),
            ))
            .mount(&mock_server)
            .await;

        let tools = vec![ToolSchema::from_parts(
            "get_weather",
            "Get weather for a city",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                }
            }),
        )];

        let response = provider
            .chat_completion(
                vec![Message::user("What's the weather in SF?")],
                Some(tools),
                Some(ToolChoice::Required),
            )
            .await
            .unwrap();

        assert!(response.tool_calls().is_some());
    }

    #[test]
    fn test_openai_message_serialization() {
        let msg = OpenAIMessage {
            role: "user".to_string(),
            content: "test message".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""role":"user""#));
        assert!(json.contains(r#""content":"test message""#));
    }

    #[test]
    fn test_chat_completion_request_serialization() {
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            tools: None,
            tool_choice: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""model":"gpt-4""#));
        assert!(json.contains(r#""role":"user""#));
        assert!(json.contains(r#""content":"hello""#));
    }

    #[test]
    fn test_chat_completion_request_with_tools() {
        let tools = vec![OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIFunction {
                name: "test_func".to_string(),
                description: "A test function".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            },
        }];

        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            tools: Some(tools),
            tool_choice: Some(ToolChoiceValue::Auto),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""tools""#));
        assert!(json.contains(r#""test_func""#));
        assert!(json.contains(r#""tool_choice":"auto""#));
    }
}
