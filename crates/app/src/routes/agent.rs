//! Agent API routes.
//!
//! Provides REST endpoints for agent file and configuration management.

use crate::routes::schemas::{CreateChatRequest, UpdateChatRequest};
use crate::runner::chat_manager::ChatManager;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use copaw_agents::{AgentFileManager, ReActAgent};
use copaw_config::{get_working_dir, load_config, save_config};
use copaw_core::agent::Agent as _;
use copaw_core::{
    llm::LLMProvider,
    message::{ContentPart, Message, MessageContent},
};
use copaw_providers::{
    AnthropicProvider, LlamaCppProvider, OllamaProvider, OpenAIProvider, ProviderRegistry,
    ProviderStore,
};
use copaw_tools::{FileTool, ShellTool};
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// State for agent routes.
#[derive(Clone)]
pub struct AgentState {
    pub file_manager: Arc<AgentFileManager>,
    pub registry: Arc<ProviderRegistry>,
    pub store: Arc<ProviderStore>,
    pub chat_manager: Arc<ChatManager>,
}

/// Error response for agent operations.
#[derive(Debug)]
pub enum AgentRouteError {
    NotFound(String),
    #[allow(dead_code)]
    BadRequest(String),
    Internal(String),
}

impl axum::response::IntoResponse for AgentRouteError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AgentRouteError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AgentRouteError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AgentRouteError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({ "detail": message }));
        (status, body).into_response()
    }
}

impl std::fmt::Display for AgentRouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRouteError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AgentRouteError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            AgentRouteError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AgentRouteError {}

/// GET /api/agent/ - Agent root info.
pub async fn agent_root() -> Json<serde_json::Value> {
    Json(json!({
        "app_name": "Friday",
        "app_description": "A helpful assistant",
    }))
}

/// GET /api/agent/health - Agent health check.
pub async fn agent_health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
    }))
}

/// POST /api/agent/process - Agent process endpoint (compatibility shim).
pub async fn agent_process(
    State(state): State<AgentState>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let stream_mode = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let response_id = Uuid::new_v4().to_string();
    let message_id = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().timestamp_millis();

    let user_text = extract_user_text(&body);
    let chat_id = get_or_create_chat_for_request(
        &state,
        &body,
        &infer_chat_name_from_request(&body, &user_text),
    )
    .await;

    if stream_mode {
        let mut stream_rx = start_active_llm_stream(state.clone(), user_text.clone())
            .await
            .ok();
        let fallback_result = if stream_rx.is_none() {
            Some(
                match run_agent_with_tools_completion(&state, &user_text).await {
                    Ok(text) => Ok(text),
                    Err(_) => run_active_llm_completion(&state, &user_text).await,
                },
            )
        } else {
            None
        };
        let body_for_save = body.clone();
        let state_for_save = state.clone();
        let chat_id_for_save = chat_id.clone();
        let user_text_for_save = user_text.clone();
        let response_id_for_stream = response_id.clone();
        let message_id_for_stream = message_id.clone();

        let events = async_stream::stream! {
            let mut sequence_number: i64 = 0;
            yield Ok::<Event, std::convert::Infallible>(Event::default().data(json!({
                "sequence_number": sequence_number,
                "object": "response",
                "id": response_id_for_stream,
                "status": "created",
                "created_at": created_at,
                "output": [],
            }).to_string()));
            sequence_number += 1;
            yield Ok::<Event, std::convert::Infallible>(Event::default().data(json!({
                "sequence_number": sequence_number,
                "object": "response",
                "id": response_id,
                "status": "in_progress",
                "created_at": created_at,
                "output": [],
            }).to_string()));
            sequence_number += 1;
            yield Ok::<Event, std::convert::Infallible>(Event::default().data(json!({
                "sequence_number": sequence_number,
                "object": "message",
                "id": message_id_for_stream,
                "type": "message",
                "role": "assistant",
                "status": "in_progress",
                "content": [],
            }).to_string()));
            sequence_number += 1;

            let mut assistant_text = String::new();
            let mut stream_error: Option<String> = None;

            if let Some(mut rx) = stream_rx.take() {
                while let Some(item) = rx.recv().await {
                    match item {
                        Ok(chunk) => {
                            if chunk.is_empty() {
                                continue;
                            }
                            assistant_text.push_str(&chunk);
                            yield Ok::<Event, std::convert::Infallible>(Event::default().data(json!({
                                "sequence_number": sequence_number,
                                "object": "content",
                                "msg_id": message_id,
                                "type": "text",
                                "text": chunk,
                                "status": "in_progress",
                                "delta": true,
                            }).to_string()));
                            sequence_number += 1;
                        }
                        Err(err) => {
                            stream_error = Some(err);
                            break;
                        }
                    }
                }
            } else if let Some(result) = fallback_result {
                match result {
                    Ok(text) => {
                        assistant_text = text;
                        for chunk in split_text_for_sse(&assistant_text, 24) {
                            if chunk.is_empty() {
                                continue;
                            }
                            yield Ok::<Event, std::convert::Infallible>(Event::default().data(json!({
                                "sequence_number": sequence_number,
                                "object": "content",
                                "msg_id": message_id,
                                "type": "text",
                                "text": chunk,
                                "status": "in_progress",
                                "delta": true,
                            }).to_string()));
                            sequence_number += 1;
                        }
                    }
                    Err(err) => {
                        stream_error = Some(err);
                    }
                }
            }

            if stream_error.is_some() || assistant_text.trim().is_empty() {
                let err = stream_error.unwrap_or_else(|| "LLM request failed".to_string());
                let completed_at = chrono::Utc::now().timestamp_millis();
                yield Ok::<Event, std::convert::Infallible>(Event::default().data(json!({
                    "sequence_number": sequence_number,
                    "object": "response",
                    "id": response_id,
                    "status": "failed",
                    "created_at": created_at,
                    "completed_at": completed_at,
                    "output": [],
                }).to_string()));
                sequence_number += 1;
                yield Ok::<Event, std::convert::Infallible>(Event::default().data(json!({
                    "sequence_number": sequence_number,
                    "object": "message",
                    "id": message_id,
                    "type": "error",
                    "role": "assistant",
                    "status": "failed",
                    "code": 400,
                    "message": err,
                    "content": [],
                }).to_string()));
            } else {
                let _ = persist_session_messages(
                    body_for_save.get("session_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default(),
                    body_for_save.get("user_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("default"),
                    &user_text_for_save,
                    &assistant_text,
                )
                .await;
                if let Some(chat_id) = chat_id_for_save.as_deref() {
                    let _ = touch_chat_updated_at(&state_for_save, chat_id).await;
                }
                let completed_at = chrono::Utc::now().timestamp_millis();

                yield Ok::<Event, std::convert::Infallible>(Event::default().data(json!({
                    "sequence_number": sequence_number,
                    "object": "message",
                    "id": message_id,
                    "type": "message",
                    "role": "assistant",
                    "status": "completed",
                    "content": [{
                        "object": "content",
                        "type": "text",
                        "status": "completed",
                        "delta": false,
                        "text": assistant_text,
                    }],
                }).to_string()));
                sequence_number += 1;
                yield Ok::<Event, std::convert::Infallible>(Event::default().data(json!({
                    "sequence_number": sequence_number,
                    "object": "response",
                    "id": response_id,
                    "status": "completed",
                    "created_at": created_at,
                    "completed_at": completed_at,
                    "output": [{
                        "object": "message",
                        "id": message_id,
                        "type": "message",
                        "role": "assistant",
                        "status": "completed",
                        "content": [{
                            "object": "content",
                            "type": "text",
                            "status": "completed",
                            "delta": false,
                            "text": assistant_text,
                        }],
                    }],
                }).to_string()));
            }
        };

        return Sse::new(events)
            .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15)))
            .into_response();
    }

    let llm_result = match run_agent_with_tools_completion(&state, &user_text).await {
        Ok(text) => Ok(text),
        Err(_) => run_active_llm_completion(&state, &user_text).await,
    };
    let assistant_text = llm_result
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_else(|_| String::new());
    if !assistant_text.is_empty() {
        let _ = persist_session_messages(
            body.get("session_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
            body.get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or("default"),
            &user_text,
            &assistant_text,
        )
        .await;
        if let Some(chat_id) = chat_id.as_deref() {
            let _ = touch_chat_updated_at(&state, chat_id).await;
        }
    }

    Json(json!({
        "object": "response",
        "id": response_id,
        "status": if assistant_text.is_empty() { "failed" } else { "completed" },
        "created_at": created_at,
        "output": [{
            "object": "message",
            "id": message_id,
            "type": if assistant_text.is_empty() { "error" } else { "message" },
            "role": "assistant",
            "status": if assistant_text.is_empty() { "failed" } else { "completed" },
            "code": if assistant_text.is_empty() { 400 } else { 200 },
            "message": if assistant_text.is_empty() {
                "LLM request failed".to_string()
            } else {
                "".to_string()
            },
            "content": [{
                "object": "content",
                "type": "text",
                "text": assistant_text,
                "status": "completed",
                "delta": false,
            }],
        }],
    }))
    .into_response()
}

fn extract_user_text(body: &serde_json::Value) -> String {
    let Some(last_msg) = body
        .get("input")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.last())
    else {
        return String::new();
    };

    if let Some(s) = last_msg.get("content").and_then(|v| v.as_str()) {
        return s.to_string();
    }

    let mut texts = Vec::new();
    if let Some(content_arr) = last_msg.get("content").and_then(|v| v.as_array()) {
        for part in content_arr {
            if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    if !text.is_empty() {
                        texts.push(text.to_string());
                    }
                }
            }
        }
    }

    texts.join("\n")
}

fn infer_chat_name_from_request(body: &Value, user_text: &str) -> String {
    let trimmed = user_text.trim();
    if trimmed.is_empty() {
        let has_input = body
            .get("input")
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false);
        if has_input {
            return "Media Message".to_string();
        }
        return "New Chat".to_string();
    }
    trimmed.chars().take(10).collect()
}

fn request_chat_scope(body: &Value) -> Option<(String, String, String)> {
    let session_id = body.get("session_id").and_then(|v| v.as_str())?;
    if session_id.trim().is_empty() {
        return None;
    }
    let user_id = body
        .get("user_id")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();
    let channel = body
        .get("channel")
        .and_then(|v| v.as_str())
        .unwrap_or("console")
        .to_string();
    Some((session_id.to_string(), user_id, channel))
}

async fn get_or_create_chat_for_request(
    state: &AgentState,
    body: &Value,
    default_name: &str,
) -> Option<String> {
    let (session_id, user_id, channel) = request_chat_scope(body)?;
    let existing = state
        .chat_manager
        .list(Some(&channel), Some(&user_id), Some(&session_id))
        .await
        .ok()
        .and_then(|mut chats| chats.pop());
    if let Some(chat) = existing {
        return Some(chat.id);
    }

    state
        .chat_manager
        .create(CreateChatRequest {
            name: default_name.to_string(),
            session_id,
            user_id,
            channel,
        })
        .await
        .ok()
        .map(|chat| chat.id)
}

async fn touch_chat_updated_at(state: &AgentState, chat_id: &str) -> Result<(), String> {
    state
        .chat_manager
        .update(
            chat_id,
            UpdateChatRequest {
                name: None,
                meta: std::collections::HashMap::new(),
            },
        )
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

async fn start_active_llm_stream(
    state: AgentState,
    user_text: String,
) -> Result<mpsc::Receiver<Result<String, String>>, String> {
    let data = state
        .store
        .load()
        .map_err(|e| format!("Failed to load provider config: {e}"))?;
    let slot = data.active_llm.clone();
    if slot.provider_id.is_empty() || slot.model.is_empty() {
        return Err("LLM model required: active_llm is not configured.".to_string());
    }

    let provider = state
        .registry
        .get(&slot.provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", slot.provider_id))?;
    let (mut base_url, api_key) = data.get_credentials(&slot.provider_id);
    if base_url.is_empty() {
        base_url = provider.default_base_url.clone();
    }

    let prompt = if user_text.is_empty() {
        "Hello".to_string()
    } else {
        user_text
    };

    let (tx, rx) = mpsc::channel::<Result<String, String>>(64);
    let provider_id = slot.provider_id.clone();
    let model = slot.model.clone();
    let is_local = provider.is_local;

    tokio::spawn(async move {
        let result = if provider_id == "ollama" {
            stream_ollama_completion(base_url, model, prompt, tx.clone()).await
        } else if provider_id == "anthropic" {
            stream_anthropic_completion(base_url, api_key, model, prompt, tx.clone()).await
        } else if is_local {
            stream_openai_compatible_completion(base_url, String::new(), model, prompt, tx.clone())
                .await
        } else {
            stream_openai_compatible_completion(base_url, api_key, model, prompt, tx.clone()).await
        };

        if let Err(err) = result {
            let _ = tx.send(Err(err)).await;
        }
    });

    Ok(rx)
}

async fn stream_openai_compatible_completion(
    base_url: String,
    api_key: String,
    model: String,
    prompt: String,
    tx: mpsc::Sender<Result<String, String>>,
) -> Result<(), String> {
    if base_url.is_empty() {
        return Err("Provider base_url is required.".to_string());
    }

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let client = Client::new();
    let mut request = client.post(url).json(&json!({
        "model": model,
        "stream": true,
        "messages": [{"role":"user","content": prompt}],
    }));
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Provider request failed: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Provider request failed: HTTP {}: {}",
            status, body
        ));
    }

    let mut bytes_stream = response.bytes_stream();
    let mut buffer = String::new();
    while let Some(chunk) = bytes_stream.next().await {
        let chunk = chunk.map_err(|e| format!("Provider stream read failed: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end_matches('\r').to_string();
            buffer = buffer[pos + 1..].to_string();
            if parse_openai_stream_line(&line, &tx).await? {
                return Ok(());
            }
        }
    }

    if !buffer.trim().is_empty() {
        let _ = parse_openai_stream_line(buffer.trim(), &tx).await?;
    }
    Ok(())
}

async fn parse_openai_stream_line(
    line: &str,
    tx: &mpsc::Sender<Result<String, String>>,
) -> Result<bool, String> {
    if line.is_empty() {
        return Ok(false);
    }
    let payload = if line.starts_with("data:") {
        line.trim_start_matches("data:").trim()
    } else {
        line.trim()
    };
    if payload == "[DONE]" {
        return Ok(true);
    }

    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|e| format!("Invalid stream payload: {e}"))?;
    let content = value
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| {
            choice
                .get("delta")
                .and_then(|delta| delta.get("content"))
                .or_else(|| choice.get("message").and_then(|m| m.get("content")))
        })
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !content.is_empty() {
        tx.send(Ok(content.to_string()))
            .await
            .map_err(|_| "Stream receiver dropped".to_string())?;
        // non-stream JSON fallback: consider this completion done
        if !line.starts_with("data:") {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn parse_anthropic_stream_line(
    line: &str,
    tx: &mpsc::Sender<Result<String, String>>,
) -> Result<bool, String> {
    if line.is_empty() || line.starts_with("event:") {
        return Ok(false);
    }
    let payload = if line.starts_with("data:") {
        line.trim_start_matches("data:").trim()
    } else {
        line.trim()
    };
    if payload == "[DONE]" {
        return Ok(true);
    }

    let value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| format!("Invalid Anthropic stream payload: {e}"))?;
    let event_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    match event_type {
        "content_block_delta" => {
            if value
                .get("delta")
                .and_then(|d| d.get("type"))
                .and_then(|v| v.as_str())
                == Some("text_delta")
            {
                let text = value
                    .get("delta")
                    .and_then(|d| d.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if !text.is_empty() {
                    tx.send(Ok(text.to_string()))
                        .await
                        .map_err(|_| "Stream receiver dropped".to_string())?;
                }
            }
            Ok(false)
        }
        // non-stream JSON fallback from /messages response
        "message" => {
            let text = value
                .get("content")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if !text.is_empty() {
                tx.send(Ok(text.to_string()))
                    .await
                    .map_err(|_| "Stream receiver dropped".to_string())?;
                return Ok(true);
            }
            Ok(false)
        }
        "message_stop" => Ok(true),
        "error" => {
            let msg = value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("Anthropic stream error")
                .to_string();
            Err(msg)
        }
        _ => Ok(false),
    }
}

async fn stream_anthropic_completion(
    base_url: String,
    api_key: String,
    model: String,
    prompt: String,
    tx: mpsc::Sender<Result<String, String>>,
) -> Result<(), String> {
    if base_url.is_empty() {
        return Err("Anthropic base_url is required.".to_string());
    }
    if api_key.is_empty() {
        return Err("Provider 'anthropic' API key is required.".to_string());
    }

    let url = format!("{}/messages", base_url.trim_end_matches('/'));
    let client = Client::new();
    let response = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": model,
            "max_tokens": 4096,
            "stream": true,
            "messages": [{"role":"user","content": prompt}],
        }))
        .send()
        .await
        .map_err(|e| format!("Anthropic request failed: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Anthropic request failed: HTTP {}: {}",
            status, body
        ));
    }

    let mut bytes_stream = response.bytes_stream();
    let mut buffer = String::new();
    while let Some(chunk) = bytes_stream.next().await {
        let chunk = chunk.map_err(|e| format!("Anthropic stream read failed: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end_matches('\r').to_string();
            buffer = buffer[pos + 1..].to_string();
            if parse_anthropic_stream_line(&line, &tx).await? {
                return Ok(());
            }
        }
    }
    if !buffer.trim().is_empty() {
        let _ = parse_anthropic_stream_line(buffer.trim(), &tx).await?;
    }
    Ok(())
}

async fn stream_ollama_completion(
    base_url: String,
    model: String,
    prompt: String,
    tx: mpsc::Sender<Result<String, String>>,
) -> Result<(), String> {
    if base_url.is_empty() {
        return Err("Ollama base_url is required.".to_string());
    }

    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let client = Client::new();
    let response = client
        .post(url)
        .json(&json!({
            "model": model,
            "stream": true,
            "messages": [{"role":"user","content": prompt}],
        }))
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama request failed: HTTP {}: {}", status, body));
    }

    let mut bytes_stream = response.bytes_stream();
    let mut buffer = String::new();
    while let Some(chunk) = bytes_stream.next().await {
        let chunk = chunk.map_err(|e| format!("Ollama stream read failed: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end_matches('\r').to_string();
            buffer = buffer[pos + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&line)
                .map_err(|e| format!("Invalid Ollama stream payload: {e}"))?;
            let content = value
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if !content.is_empty() {
                tx.send(Ok(content.to_string()))
                    .await
                    .map_err(|_| "Stream receiver dropped".to_string())?;
            }
            if value.get("done").and_then(|v| v.as_bool()) == Some(true) {
                return Ok(());
            }
        }
    }

    if !buffer.trim().is_empty() {
        let value: serde_json::Value = serde_json::from_str(buffer.trim())
            .map_err(|e| format!("Invalid Ollama stream payload: {e}"))?;
        let content = value
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if !content.is_empty() {
            tx.send(Ok(content.to_string()))
                .await
                .map_err(|_| "Stream receiver dropped".to_string())?;
        }
    }

    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push_str("--");
        } else {
            out.push(c);
        }
    }
    out
}

fn session_file_path(session_id: &str, user_id: &str) -> std::path::PathBuf {
    let safe_sid = sanitize_filename(session_id);
    let safe_uid = sanitize_filename(user_id);
    let file_name = if safe_uid.is_empty() {
        format!("{safe_sid}.json")
    } else {
        format!("{safe_uid}_{safe_sid}.json")
    };
    get_working_dir().join("sessions").join(file_name)
}

async fn persist_session_messages(
    session_id: &str,
    user_id: &str,
    user_text: &str,
    assistant_text: &str,
) -> Result<(), String> {
    if session_id.trim().is_empty() {
        return Ok(());
    }

    let path = session_file_path(session_id, user_id);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed creating session dir: {e}"))?;
    }

    let mut state: Value = match tokio::fs::read_to_string(&path).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| json!({})),
        Err(_) => json!({}),
    };
    if !state.is_object() {
        state = json!({});
    }

    if state.get("agent").is_none() {
        state["agent"] = json!({});
    }
    if state
        .get("agent")
        .and_then(|a| a.get("memory"))
        .and_then(Value::as_array)
        .is_none()
    {
        state["agent"]["memory"] = json!([]);
    }
    let memories = state["agent"]["memory"]
        .as_array_mut()
        .ok_or_else(|| "Invalid session memory format".to_string())?;

    if !user_text.trim().is_empty() {
        memories.push(json!({
            "name": "user",
            "role": "user",
            "content": [{"type":"text","text": user_text}],
        }));
    }

    if !assistant_text.trim().is_empty() {
        memories.push(json!({
            "name": "assistant",
            "role": "assistant",
            "content": [{"type":"text","text": assistant_text}],
        }));
    }

    let payload = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed serializing session: {e}"))?;
    tokio::fs::write(&path, payload)
        .await
        .map_err(|e| format!("Failed writing session: {e}"))?;
    Ok(())
}

fn message_to_text(message: &Message) -> String {
    match message.content() {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn split_text_for_sse(text: &str, chunk_size: usize) -> Vec<String> {
    if text.is_empty() || chunk_size == 0 {
        return vec![text.to_string()];
    }
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let end = std::cmp::min(i + chunk_size, chars.len());
        chunks.push(chars[i..end].iter().collect());
        i = end;
    }
    chunks
}

async fn run_agent_with_tools_completion(
    state: &AgentState,
    user_text: &str,
) -> Result<String, String> {
    let data = state
        .store
        .load()
        .map_err(|e| format!("Failed to load provider config: {e}"))?;
    let slot = &data.active_llm;
    if slot.provider_id.is_empty() || slot.model.is_empty() {
        return Err("LLM model required: active_llm is not configured.".to_string());
    }

    let provider = state
        .registry
        .get(&slot.provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", slot.provider_id))?;
    let (mut base_url, api_key) = data.get_credentials(&slot.provider_id);
    if base_url.is_empty() {
        base_url = provider.default_base_url.clone();
    }

    let prompt = if user_text.is_empty() {
        "Hello"
    } else {
        user_text
    };

    if slot.provider_id == "ollama" {
        if base_url.is_empty() {
            return Err("Ollama base_url is required.".to_string());
        }
        return run_react_with_provider(
            Arc::new(OllamaProvider::new(base_url, &slot.model)),
            prompt,
            "ollama-react",
        )
        .await;
    }

    if slot.provider_id == "anthropic" {
        if base_url.is_empty() {
            return Err("Anthropic base_url is required.".to_string());
        }
        if api_key.is_empty() {
            return Err("Provider 'anthropic' API key is required.".to_string());
        }
        return run_react_with_provider(
            Arc::new(AnthropicProvider::new(base_url, api_key, &slot.model)),
            prompt,
            "anthropic-react",
        )
        .await;
    }

    if provider.is_local {
        if base_url.is_empty() {
            return Err(format!(
                "Local provider '{}' requires base_url in current Rust implementation.",
                slot.provider_id
            ));
        }
        return run_react_with_provider(
            Arc::new(LlamaCppProvider::new(base_url, &slot.model)),
            prompt,
            "local-react",
        )
        .await;
    }

    if api_key.is_empty() {
        return Err(format!(
            "Provider '{}' API key is required.",
            slot.provider_id
        ));
    }

    run_react_with_provider(
        Arc::new(OpenAIProvider::new(base_url, api_key, &slot.model)),
        prompt,
        "openai-react",
    )
    .await
}

async fn run_react_with_provider<P>(
    provider: Arc<P>,
    user_text: &str,
    agent_name: &str,
) -> Result<String, String>
where
    P: LLMProvider + Send + Sync + 'static,
{
    let sys_prompt = "You are CoPaw, a desktop automation assistant. \
You can use tools to execute shell commands and file operations on user request. \
When the user asks to operate files or system tasks, use tools instead of refusing.";
    let mut agent = ReActAgent::new(agent_name, provider, sys_prompt);
    agent
        .add_tool(Box::new(ShellTool::new(get_working_dir())))
        .map_err(|e| format!("Failed to register shell tool: {e}"))?;
    agent
        .add_tool(Box::new(FileTool::unrestricted()))
        .map_err(|e| format!("Failed to register file tool: {e}"))?;

    let response = agent
        .reply(Message::user(user_text))
        .await
        .map_err(|e| format!("Agent failed: {e}"))?;
    let text = message_to_text(&response);
    if text.trim().is_empty() {
        Err("Agent returned empty response.".to_string())
    } else {
        Ok(text)
    }
}

async fn run_active_llm_completion(state: &AgentState, user_text: &str) -> Result<String, String> {
    let data = state
        .store
        .load()
        .map_err(|e| format!("Failed to load provider config: {e}"))?;
    let slot = &data.active_llm;
    if slot.provider_id.is_empty() || slot.model.is_empty() {
        return Err("LLM model required: active_llm is not configured.".to_string());
    }

    let provider = state
        .registry
        .get(&slot.provider_id)
        .ok_or_else(|| format!("Provider '{}' not found", slot.provider_id))?;
    let (mut base_url, api_key) = data.get_credentials(&slot.provider_id);
    if base_url.is_empty() {
        base_url = provider.default_base_url.clone();
    }
    let user_message = Message::user(if user_text.is_empty() {
        "Hello"
    } else {
        user_text
    });

    let llm_response = if slot.provider_id == "ollama" {
        if base_url.is_empty() {
            return Err("Ollama base_url is required.".to_string());
        }
        OllamaProvider::new(base_url, &slot.model)
            .chat_completion(vec![user_message], None, None)
            .await
            .map_err(|e| format!("Ollama request failed: {e}"))?
    } else if slot.provider_id == "anthropic" {
        if base_url.is_empty() {
            return Err("Anthropic base_url is required.".to_string());
        }
        if api_key.is_empty() {
            return Err("Provider 'anthropic' API key is required.".to_string());
        }
        AnthropicProvider::new(base_url, api_key, &slot.model)
            .chat_completion(vec![user_message], None, None)
            .await
            .map_err(|e| format!("Anthropic request failed: {e}"))?
    } else if provider.is_local {
        if base_url.is_empty() {
            return Err(format!(
                "Local provider '{}' requires base_url in current Rust implementation.",
                slot.provider_id
            ));
        }
        LlamaCppProvider::new(base_url, &slot.model)
            .chat_completion(vec![user_message], None, None)
            .await
            .map_err(|e| format!("Local model request failed: {e}"))?
    } else {
        if api_key.is_empty() {
            return Err(format!(
                "Provider '{}' API key is required.",
                slot.provider_id
            ));
        }
        OpenAIProvider::new(base_url, api_key, &slot.model)
            .chat_completion(vec![user_message], None, None)
            .await
            .map_err(|e| format!("Provider request failed: {e}"))?
    };

    let text = message_to_text(llm_response.content());
    if text.is_empty() {
        Err("LLM returned empty response.".to_string())
    } else {
        Ok(text)
    }
}

/// GET /api/agent/admin/status - Agent runtime status.
pub async fn get_process_status() -> Json<serde_json::Value> {
    Json(json!({
        "status": "running",
        "mode": "compatibility",
    }))
}

/// POST /api/agent/shutdown - Compatibility shutdown endpoint.
pub async fn shutdown_simple() -> Json<serde_json::Value> {
    Json(json!({
        "success": true,
        "message": "Shutdown requested (compatibility endpoint).",
    }))
}

/// POST /api/agent/admin/shutdown - Compatibility admin shutdown endpoint.
pub async fn shutdown_admin() -> Json<serde_json::Value> {
    Json(json!({
        "success": true,
        "message": "Admin shutdown requested (compatibility endpoint).",
    }))
}

/// GET /api/agent/files - List working files.
pub async fn list_working_files(
    State(state): State<AgentState>,
) -> Result<Json<Vec<crate::routes::schemas::MdFileInfo>>, AgentRouteError> {
    let files = state
        .file_manager
        .list_working_mds()
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    let file_infos: Vec<crate::routes::schemas::MdFileInfo> = files
        .into_iter()
        .map(|f| crate::routes::schemas::MdFileInfo {
            filename: f.filename,
            path: f.path.display().to_string(),
            size: f.size,
            created_time: f.created_time,
            modified_time: f.modified_time,
        })
        .collect();

    Ok(Json(file_infos))
}

/// GET /api/agent/files/:name - Read a working file.
pub async fn read_working_file(
    State(state): State<AgentState>,
    Path(md_name): Path<String>,
) -> Result<Json<crate::routes::schemas::MdFileContent>, AgentRouteError> {
    let content = state
        .file_manager
        .read_working_md(&md_name)
        .map_err(|e| match e {
            copaw_agents::AgentFileManagerError::FileNotFound(_) => {
                AgentRouteError::NotFound(format!("File not found: {}", md_name))
            }
            _ => AgentRouteError::Internal(e.to_string()),
        })?;

    Ok(Json(crate::routes::schemas::MdFileContent { content }))
}

/// PUT /api/agent/files/:name - Write a working file.
pub async fn write_working_file(
    State(state): State<AgentState>,
    Path(md_name): Path<String>,
    Json(req): Json<crate::routes::schemas::MdFileContent>,
) -> Result<Json<serde_json::Value>, AgentRouteError> {
    state
        .file_manager
        .write_working_md(&md_name, &req.content)
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "written": true })))
}

/// GET /api/agent/memory - List memory files.
pub async fn list_memory_files(
    State(state): State<AgentState>,
) -> Result<Json<Vec<crate::routes::schemas::MdFileInfo>>, AgentRouteError> {
    let files = state
        .file_manager
        .list_memory_mds()
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    let file_infos: Vec<crate::routes::schemas::MdFileInfo> = files
        .into_iter()
        .map(|f| crate::routes::schemas::MdFileInfo {
            filename: f.filename,
            path: f.path.display().to_string(),
            size: f.size,
            created_time: f.created_time,
            modified_time: f.modified_time,
        })
        .collect();

    Ok(Json(file_infos))
}

/// GET /api/agent/memory/:name - Read a memory file.
pub async fn read_memory_file(
    State(state): State<AgentState>,
    Path(md_name): Path<String>,
) -> Result<Json<crate::routes::schemas::MdFileContent>, AgentRouteError> {
    let content = state
        .file_manager
        .read_memory_md(&md_name)
        .map_err(|e| match e {
            copaw_agents::AgentFileManagerError::FileNotFound(_) => {
                AgentRouteError::NotFound(format!("Memory file not found: {}", md_name))
            }
            _ => AgentRouteError::Internal(e.to_string()),
        })?;

    Ok(Json(crate::routes::schemas::MdFileContent { content }))
}

/// PUT /api/agent/memory/:name - Write a memory file.
pub async fn write_memory_file(
    State(state): State<AgentState>,
    Path(md_name): Path<String>,
    Json(req): Json<crate::routes::schemas::MdFileContent>,
) -> Result<Json<serde_json::Value>, AgentRouteError> {
    state
        .file_manager
        .write_memory_md(&md_name, &req.content)
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "written": true })))
}

/// GET /api/agent/running-config - Get agent running config.
pub async fn get_running_config() -> Result<Json<copaw_config::AgentsRunningConfig>, AgentRouteError>
{
    let config = load_config(None)
        .await
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    Ok(Json(config.agents.running))
}

/// PUT /api/agent/running-config - Update agent running config.
pub async fn update_running_config(
    Json(running_config): Json<copaw_config::AgentsRunningConfig>,
) -> Result<Json<copaw_config::AgentsRunningConfig>, AgentRouteError> {
    let mut config = load_config(None)
        .await
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    config.agents.running = running_config.clone();

    save_config(&config, None)
        .await
        .map_err(|e| AgentRouteError::Internal(e.to_string()))?;

    Ok(Json(running_config))
}

/// Create the agent router.
pub fn create_agent_router() -> axum::Router<AgentState> {
    use axum::routing::*;

    axum::Router::new()
        .route("/", get(agent_root))
        .route("/health", get(agent_health))
        .route("/process", post(agent_process))
        .route("/admin/status", get(get_process_status))
        .route("/shutdown", post(shutdown_simple))
        .route("/admin/shutdown", post(shutdown_admin))
        .route("/files", get(list_working_files))
        .route(
            "/files/:md_name",
            get(read_working_file).put(write_working_file),
        )
        .route("/memory", get(list_memory_files))
        .route(
            "/memory/:md_name",
            get(read_memory_file).put(write_memory_file),
        )
        .route(
            "/running-config",
            get(get_running_config).put(update_running_config),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::header, response::IntoResponse, routing::post, Router};
    use copaw_agents::AgentFileManager;
    use tempfile::TempDir;
    use tokio::net::TcpListener;

    struct TestState {
        state: AgentState,
        _temp_dir: TempDir,
    }

    async fn create_test_state() -> TestState {
        let temp_dir = TempDir::new().unwrap();
        let file_manager = AgentFileManager::with_working_dir(temp_dir.path()).unwrap();
        let registry = Arc::new(ProviderRegistry::new());
        let providers_path = temp_dir.path().join("providers.json");
        let store = Arc::new(ProviderStore::new(providers_path, registry.clone()));
        let chats_repo_path = temp_dir.path().join("chats.json");
        let chats_repo = crate::repo::ChatRepository::with_path(chats_repo_path).unwrap();
        let chat_manager = Arc::new(crate::runner::chat_manager::ChatManager::with_repository(
            chats_repo,
        ));
        TestState {
            state: AgentState {
                file_manager: Arc::new(file_manager),
                registry,
                store,
                chat_manager,
            },
            _temp_dir: temp_dir,
        }
    }

    async fn start_mock_openai_server() -> String {
        async fn completion_handler() -> impl IntoResponse {
            Json(json!({
                "id": "chatcmpl-test",
                "object": "chat.completion",
                "created": 1772616043u64,
                "model": "gpt-4o-mini",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 2,
                    "total_tokens": 12
                }
            }))
        }

        let app = Router::new().route("/chat/completions", post(completion_handler));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{}", addr)
    }

    async fn start_mock_anthropic_server() -> String {
        async fn completion_handler() -> impl IntoResponse {
            Json(json!({
                "id": "msg_test",
                "type": "message",
                "role": "assistant",
                "content": [{"type":"text","text":"Hello from Claude"}],
                "model": "claude-3-7-sonnet-latest",
                "stop_reason": "end_turn",
                "usage": {
                    "input_tokens": 10,
                    "output_tokens": 5
                }
            }))
        }

        let app = Router::new().route("/messages", post(completion_handler));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{}", addr)
    }

    async fn start_mock_anthropic_stream_server() -> String {
        async fn completion_handler() -> impl IntoResponse {
            (
                [(header::CONTENT_TYPE, "text/event-stream")],
                "event: message_start\n\
data: {\"type\":\"message_start\"}\n\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\"}\n\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello \"}}\n\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"from Claude\"}}\n\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\n",
            )
        }

        let app = Router::new().route("/messages", post(completion_handler));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{}", addr)
    }

    #[tokio::test]
    async fn test_list_working_files_empty() {
        let test_state = create_test_state().await;
        let result = list_working_files(State(test_state.state)).await;
        assert!(result.is_ok());
        let files = result.unwrap().0;
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_write_and_read_working_file() {
        let test_state = create_test_state().await;

        let req = crate::routes::schemas::MdFileContent {
            content: "# Test Content".to_string(),
        };

        let write_result = write_working_file(
            State(test_state.state.clone()),
            Path("test".to_string()),
            Json(req.clone()),
        )
        .await;
        assert!(write_result.is_ok());

        let read_result =
            read_working_file(State(test_state.state), Path("test".to_string())).await;
        assert!(read_result.is_ok());
        let content = read_result.unwrap().0;
        assert_eq!(content.content, "# Test Content");
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let test_state = create_test_state().await;
        let result =
            read_working_file(State(test_state.state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_memory_files_empty() {
        let test_state = create_test_state().await;
        let result = list_memory_files(State(test_state.state)).await;
        assert!(result.is_ok());
        let files = result.unwrap().0;
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn test_agent_root() {
        let Json(value) = agent_root().await;
        assert_eq!(
            value.get("app_name").and_then(|v| v.as_str()),
            Some("Friday")
        );
    }

    #[tokio::test]
    async fn test_agent_health() {
        let Json(value) = agent_health().await;
        assert_eq!(value.get("status").and_then(|v| v.as_str()), Some("ok"));
    }

    #[tokio::test]
    async fn test_agent_process_non_stream() {
        let test_state = create_test_state().await;
        let body = json!({
            "input": [{"role":"user","content":"hello"}],
            "stream": false
        });
        let response = agent_process(State(test_state.state), Json(body)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_agent_process_auto_creates_chat_with_first_message_title() {
        let test_state = create_test_state().await;
        let body = json!({
            "input": [{"role":"user","content":"这是第一条消息标题测试"}],
            "session_id": "console:u-test",
            "user_id": "u-test",
            "channel": "console",
            "stream": false
        });

        let response = agent_process(State(test_state.state.clone()), Json(body)).await;
        assert_eq!(response.status(), StatusCode::OK);

        let chats = test_state
            .state
            .chat_manager
            .list(Some("console"), Some("u-test"), Some("console:u-test"))
            .await
            .unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].name, "这是第一条消息标题测");
    }

    #[tokio::test]
    async fn test_agent_process_stream() {
        let test_state = create_test_state().await;
        let body = json!({
            "input": [{
                "role":"user",
                "content":[{"type":"text","text":"hello"}]
            }],
            "stream": true
        });
        let response = agent_process(State(test_state.state), Json(body)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(content_type.contains("text/event-stream"));
    }

    #[tokio::test]
    async fn test_agent_process_stream_completed_response_has_output_content() {
        let test_state = create_test_state().await;
        let base_url = start_mock_openai_server().await;

        test_state
            .state
            .store
            .update_settings("openai", Some("sk-test".to_string()), Some(base_url), None)
            .unwrap();
        test_state
            .state
            .store
            .set_active_llm("openai", "gpt-4o-mini")
            .unwrap();

        let body = json!({
            "input": [{"role":"user","content":"hello"}],
            "stream": true
        });
        let response = agent_process(State(test_state.state), Json(body)).await;
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let s = String::from_utf8(bytes.to_vec()).unwrap();

        let mut completed_response: Option<serde_json::Value> = None;
        let mut saw_empty_completed_output = false;
        for line in s.lines() {
            let line = line.trim();
            if !line.starts_with("data:") {
                continue;
            }
            let payload = line.trim_start_matches("data:").trim();
            if payload.is_empty() || payload == "[DONE]" {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(payload).unwrap();
            if v.get("object").and_then(|x| x.as_str()) == Some("response")
                && v.get("status").and_then(|x| x.as_str()) == Some("completed")
            {
                let empty_output = v
                    .get("output")
                    .and_then(|o| o.as_array())
                    .map(|arr| arr.is_empty())
                    .unwrap_or(true);
                if empty_output {
                    saw_empty_completed_output = true;
                }
                completed_response = Some(v);
            }
        }

        let completed_response = completed_response.expect("missing completed response event");
        assert!(
            !saw_empty_completed_output,
            "completed response should not have empty output"
        );
        assert!(
            completed_response
                .get("completed_at")
                .and_then(|v| v.as_i64())
                .is_some(),
            "completed response should include completed_at"
        );
        assert!(
            completed_response
                .get("sequence_number")
                .and_then(|v| v.as_i64())
                .is_some(),
            "completed response should include sequence_number"
        );
        let output = completed_response
            .get("output")
            .and_then(|v| v.as_array())
            .expect("completed response should contain output array");
        assert!(
            !output.is_empty(),
            "completed response output should not be empty"
        );
        let content = output[0]
            .get("content")
            .and_then(|v| v.as_array())
            .expect("completed response output[0] should contain content array");
        assert!(
            !content.is_empty(),
            "completed response output[0].content should not be empty"
        );
    }

    #[tokio::test]
    async fn test_agent_process_non_stream_with_anthropic_active() {
        let test_state = create_test_state().await;
        let base_url = start_mock_anthropic_server().await;

        test_state
            .state
            .store
            .update_settings(
                "anthropic",
                Some("sk-ant-test".to_string()),
                Some(base_url),
                None,
            )
            .unwrap();
        test_state
            .state
            .store
            .set_active_llm("anthropic", "claude-3-7-sonnet-latest")
            .unwrap();

        let body = json!({
            "input": [{"role":"user","content":"hello anthropic"}],
            "stream": false
        });
        let response = agent_process(State(test_state.state), Json(body)).await;
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v.get("status").and_then(|x| x.as_str()), Some("completed"));
        let text = v
            .get("output")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default();
        assert_eq!(text, "Hello from Claude");
    }

    #[tokio::test]
    async fn test_agent_process_stream_with_anthropic_active() {
        let test_state = create_test_state().await;
        let base_url = start_mock_anthropic_stream_server().await;

        test_state
            .state
            .store
            .update_settings(
                "anthropic",
                Some("sk-ant-test".to_string()),
                Some(base_url),
                None,
            )
            .unwrap();
        test_state
            .state
            .store
            .set_active_llm("anthropic", "claude-3-7-sonnet-latest")
            .unwrap();

        let body = json!({
            "input": [{"role":"user","content":"hello anthropic"}],
            "stream": true
        });
        let response = agent_process(State(test_state.state), Json(body)).await;
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let s = String::from_utf8(bytes.to_vec()).unwrap();

        let mut completed_response: Option<serde_json::Value> = None;
        for line in s.lines() {
            let line = line.trim();
            if !line.starts_with("data:") {
                continue;
            }
            let payload = line.trim_start_matches("data:").trim();
            if payload.is_empty() || payload == "[DONE]" {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(payload).unwrap();
            if v.get("object").and_then(|x| x.as_str()) == Some("response")
                && v.get("status").and_then(|x| x.as_str()) == Some("completed")
            {
                completed_response = Some(v);
            }
        }

        let completed_response = completed_response.expect("missing completed response event");
        let text = completed_response
            .get("output")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default();
        assert_eq!(text, "Hello from Claude");
    }

    #[tokio::test]
    async fn test_write_and_read_memory_file() {
        let test_state = create_test_state().await;

        let req = crate::routes::schemas::MdFileContent {
            content: "# Memory Content".to_string(),
        };

        let write_result = write_memory_file(
            State(test_state.state.clone()),
            Path("mem_test".to_string()),
            Json(req.clone()),
        )
        .await;
        assert!(write_result.is_ok());

        let read_result =
            read_memory_file(State(test_state.state), Path("mem_test".to_string())).await;
        assert!(read_result.is_ok());
        let content = read_result.unwrap().0;
        assert_eq!(content.content, "# Memory Content");
    }
}
