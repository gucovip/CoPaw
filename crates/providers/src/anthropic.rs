//! Anthropic provider implementation

use async_trait::async_trait;
use copaw_core::llm::{
    LLMError, LLMProvider, LLMResponse, ToolCall, ToolChoice, ToolSchema, Usage,
};
use copaw_core::message::{ContentPart, Message, MessageContent, MessageRole, ToolOutputVariant};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct AnthropicTextBlock {
    #[serde(rename = "type")]
    kind: String,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct AnthropicToolUseBlock {
    #[serde(rename = "type")]
    kind: String,
    id: String,
    name: String,
    input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct AnthropicToolResultBlock {
    #[serde(rename = "type")]
    kind: String,
    tool_use_id: String,
    content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum AnthropicContentBlock {
    Text(AnthropicTextBlock),
    ToolUse(AnthropicToolUseBlock),
    ToolResult(AnthropicToolResultBlock),
}

#[derive(Debug, Clone, Serialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContentBlock>,
}

#[derive(Debug, Clone, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
struct AnthropicToolChoice {
    #[serde(rename = "type")]
    choice_type: String,
}

#[derive(Debug, Clone, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<AnthropicToolChoice>,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ResponseContentBlock>,
    usage: ResponseUsage,
}

#[derive(Debug, Deserialize)]
struct ResponseUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ResponseContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct AnthropicErrorResponse {
    error: AnthropicError,
}

#[derive(Debug, Deserialize)]
struct AnthropicError {
    #[serde(rename = "type")]
    _error_type: Option<String>,
    message: String,
}

pub struct AnthropicProvider {
    client: Client,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl AnthropicProvider {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    fn convert_tools(&self, tools: Option<Vec<ToolSchema>>) -> Option<Vec<AnthropicTool>> {
        tools.map(|items| {
            items
                .into_iter()
                .map(|item| AnthropicTool {
                    name: item.function.name,
                    description: item.function.description,
                    input_schema: item.function.parameters,
                })
                .collect()
        })
    }

    fn convert_tool_choice(&self, choice: Option<ToolChoice>) -> Option<AnthropicToolChoice> {
        match choice.unwrap_or(ToolChoice::None) {
            ToolChoice::Auto => Some(AnthropicToolChoice {
                choice_type: "auto".to_string(),
            }),
            ToolChoice::Required => Some(AnthropicToolChoice {
                choice_type: "any".to_string(),
            }),
            ToolChoice::None => None,
        }
    }

    fn convert_messages(&self, messages: Vec<Message>) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system_parts = Vec::new();
        let mut converted = Vec::new();

        for msg in messages {
            match msg.role() {
                MessageRole::System => {
                    let t = message_text(&msg);
                    if !t.is_empty() {
                        system_parts.push(t);
                    }
                }
                MessageRole::User => converted.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: content_to_blocks(msg.content(), "user"),
                }),
                MessageRole::Assistant => converted.push(AnthropicMessage {
                    role: "assistant".to_string(),
                    content: content_to_blocks(msg.content(), "assistant"),
                }),
                MessageRole::Tool => converted.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: content_to_blocks(msg.content(), "tool"),
                }),
            }
        }

        let system = if system_parts.is_empty() {
            None
        } else {
            Some(system_parts.join("\n\n"))
        };
        (system, converted)
    }
}

fn message_text(msg: &Message) -> String {
    match msg.content() {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.clone()),
                ContentPart::Thinking { thinking } => Some(thinking.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn content_to_blocks(content: &MessageContent, role: &str) -> Vec<AnthropicContentBlock> {
    match content {
        MessageContent::Text(text) => vec![AnthropicContentBlock::Text(AnthropicTextBlock {
            kind: "text".to_string(),
            text: text.clone(),
        })],
        MessageContent::Parts(parts) => {
            let mut blocks = Vec::new();
            for part in parts {
                match part {
                    ContentPart::Text { text } => {
                        blocks.push(AnthropicContentBlock::Text(AnthropicTextBlock {
                            kind: "text".to_string(),
                            text: text.clone(),
                        }));
                    }
                    ContentPart::Thinking { thinking } => {
                        blocks.push(AnthropicContentBlock::Text(AnthropicTextBlock {
                            kind: "text".to_string(),
                            text: format!("[thinking] {}", thinking),
                        }));
                    }
                    ContentPart::ToolUse {
                        id, name, input, ..
                    } if role == "assistant" => {
                        blocks.push(AnthropicContentBlock::ToolUse(AnthropicToolUseBlock {
                            kind: "tool_use".to_string(),
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        }));
                    }
                    ContentPart::ToolResult { id, output, .. }
                        if role == "tool" || role == "user" =>
                    {
                        let content_text = match output {
                            ToolOutputVariant::Text(text) => text.clone(),
                            ToolOutputVariant::Blocks(nested) => nested
                                .iter()
                                .filter_map(|np| match np {
                                    ContentPart::Text { text } => Some(text.clone()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n"),
                        };
                        blocks.push(AnthropicContentBlock::ToolResult(
                            AnthropicToolResultBlock {
                                kind: "tool_result".to_string(),
                                tool_use_id: id.clone(),
                                content: content_text,
                            },
                        ));
                    }
                    _ => {}
                }
            }

            if blocks.is_empty() {
                vec![AnthropicContentBlock::Text(AnthropicTextBlock {
                    kind: "text".to_string(),
                    text: String::new(),
                })]
            } else {
                blocks
            }
        }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolSchema>>,
        tool_choice: Option<ToolChoice>,
    ) -> Result<LLMResponse, LLMError> {
        let url = format!("{}/messages", self.base_url.trim_end_matches('/'));
        let (system, anthropic_messages) = self.convert_messages(messages);
        let request = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages: anthropic_messages,
            system,
            tools: self.convert_tools(tools),
            tool_choice: self.convert_tool_choice(tool_choice),
        };

        let response = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            if let Ok(error_resp) = serde_json::from_str::<AnthropicErrorResponse>(&body) {
                let message = error_resp.error.message;
                return match status.as_u16() {
                    401 | 403 => Err(LLMError::AuthFailed(message)),
                    429 => Err(LLMError::RateLimit(message)),
                    _ => Err(LLMError::RequestFailed(format!("{}: {}", status, message))),
                };
            }
            return Err(LLMError::RequestFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let parsed: MessagesResponse = serde_json::from_str(&body).map_err(|e| {
            LLMError::InvalidResponse(format!("Failed to parse Anthropic response: {}", e))
        })?;

        let mut parts = Vec::new();
        let mut tool_calls = Vec::new();
        for block in parsed.content {
            match block {
                ResponseContentBlock::Text { text } => parts.push(ContentPart::text(text)),
                ResponseContentBlock::ToolUse { id, name, input } => {
                    parts.push(ContentPart::tool_use(
                        id.clone(),
                        name.clone(),
                        input.clone(),
                    ));
                    tool_calls.push(ToolCall::new(id, name, input));
                }
                ResponseContentBlock::Other => {}
            }
        }

        let content = if parts.is_empty() {
            Message::assistant("")
        } else {
            Message::assistant(parts)
        };
        let usage = Usage::new(parsed.usage.input_tokens, parsed.usage.output_tokens);
        if tool_calls.is_empty() {
            Ok(LLMResponse::new(content, usage))
        } else {
            Ok(LLMResponse::with_tool_calls(content, tool_calls, usage))
        }
    }

    fn count_tokens(&self, messages: &[Message]) -> usize {
        let total_chars: usize = messages.iter().map(message_text).map(|s| s.len()).sum();
        (total_chars / 4) + (messages.len() * 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::{matchers, Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_provider_creation() {
        let p = AnthropicProvider::new(
            "https://api.anthropic.com/v1",
            "sk-ant",
            "claude-3-7-sonnet-latest",
        );
        assert_eq!(p.base_url, "https://api.anthropic.com/v1");
        assert_eq!(p.api_key, "sk-ant");
        assert_eq!(p.model, "claude-3-7-sonnet-latest");
    }

    #[tokio::test]
    async fn test_chat_completion_success_text() {
        let server = MockServer::start().await;
        let provider = AnthropicProvider::new(server.uri(), "sk-ant", "claude-3-7-sonnet-latest");

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "content": [{"type":"text","text":"Hello from Claude"}],
                "model": "claude-3-7-sonnet-latest",
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 10, "output_tokens": 5}
            })))
            .mount(&server)
            .await;

        let resp = provider
            .chat_completion(vec![Message::user("hello")], None, None)
            .await
            .unwrap();
        let text = resp
            .content()
            .content()
            .to_parts()
            .into_iter()
            .find_map(|p| match p {
                ContentPart::Text { text } => Some(text),
                _ => None,
            });
        assert_eq!(text.as_deref(), Some("Hello from Claude"));
        assert!(resp.tool_calls().is_none());
    }

    #[tokio::test]
    async fn test_chat_completion_success_tool_use() {
        let server = MockServer::start().await;
        let provider = AnthropicProvider::new(server.uri(), "sk-ant", "claude-3-7-sonnet-latest");

        Mock::given(matchers::method("POST"))
            .and(matchers::path("/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg_2",
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type":"tool_use",
                    "id":"toolu_1",
                    "name":"execute_shell_command",
                    "input":{"command":"pwd"}
                }],
                "model": "claude-3-7-sonnet-latest",
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 20, "output_tokens": 7}
            })))
            .mount(&server)
            .await;

        let resp = provider
            .chat_completion(
                vec![Message::user("run pwd")],
                Some(vec![ToolSchema::from_parts(
                    "execute_shell_command",
                    "Run shell command",
                    json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}),
                )]),
                Some(ToolChoice::Auto),
            )
            .await
            .unwrap();
        let calls = resp.tool_calls().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id(), "toolu_1");
        assert_eq!(calls[0].name(), "execute_shell_command");
    }
}
