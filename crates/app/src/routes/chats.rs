//! Chat API routes.
//!
//! Provides REST endpoints for chat management.

use crate::routes::schemas::{
    BatchDeleteRequest, ChatHistory, ChatListQuery, ChatMessage, ChatSpec, CreateChatRequest,
    UpdateChatRequest,
};
use crate::runner::chat_manager::{ChatManager, ChatManagerError};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use copaw_config::get_working_dir;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// State for chat routes.
#[derive(Clone)]
pub struct ChatsState {
    pub manager: Arc<ChatManager>,
}

/// Error response for chat operations.
#[derive(Debug)]
pub enum ChatRouteError {
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl From<ChatManagerError> for ChatRouteError {
    fn from(err: ChatManagerError) -> Self {
        match err {
            ChatManagerError::ChatNotFound(msg) => ChatRouteError::NotFound(msg),
            ChatManagerError::InvalidInput(msg) => ChatRouteError::BadRequest(msg),
            ChatManagerError::Repository(e) => ChatRouteError::Internal(e.to_string()),
        }
    }
}

impl axum::response::IntoResponse for ChatRouteError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ChatRouteError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ChatRouteError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ChatRouteError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({ "detail": message }));
        (status, body).into_response()
    }
}

/// GET /api/chats - List chats with optional filters.
pub async fn list_chats(
    State(state): State<ChatsState>,
    Query(query): Query<ChatListQuery>,
) -> Result<Json<Vec<ChatSpec>>, ChatRouteError> {
    let chats = state
        .manager
        .list(
            query.channel.as_deref(),
            query.user_id.as_deref(),
            query.session_id.as_deref(),
        )
        .await?;

    Ok(Json(chats))
}

/// POST /api/chats - Create a new chat.
pub async fn create_chat(
    State(state): State<ChatsState>,
    Json(req): Json<CreateChatRequest>,
) -> Result<Json<ChatSpec>, ChatRouteError> {
    let chat = state.manager.create(req).await?;
    Ok(Json(chat))
}

/// POST /api/chats/batch-delete - Batch delete chats.
pub async fn batch_delete_chats(
    State(state): State<ChatsState>,
    Json(req): Json<BatchDeletePayload>,
) -> Result<Json<Value>, ChatRouteError> {
    let ids = match req {
        BatchDeletePayload::Object(payload) => payload.ids,
        BatchDeletePayload::Array(ids) => ids,
    };
    let deleted = state.manager.delete_batch(ids).await?;
    Ok(Json(
        serde_json::json!({ "success": true, "deleted_count": deleted, "deleted": deleted }),
    ))
}

/// GET /api/chats/:chat_id - Get chat history.
pub async fn get_chat(
    State(state): State<ChatsState>,
    Path(chat_id): Path<String>,
) -> Result<Json<ChatHistory>, ChatRouteError> {
    let chat = state.manager.get(&chat_id).await?;
    let history = ChatHistory {
        messages: load_session_messages(&chat.session_id, &chat.user_id).await,
    };

    Ok(Json(history))
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

fn build_session_file_candidates(session_id: &str, user_id: &str) -> Vec<PathBuf> {
    let safe_sid = sanitize_filename(session_id);
    let safe_uid = sanitize_filename(user_id);

    let mut file_names = vec![format!("{safe_sid}.json"), format!("{session_id}.json")];
    if !safe_uid.is_empty() {
        file_names.insert(0, format!("{safe_uid}_{safe_sid}.json"));
        file_names.push(format!("{user_id}_{session_id}.json"));
    }

    let working_dir = get_working_dir();
    let base_dirs = vec![
        working_dir.join("sessions"),
        working_dir.join("chats").join("sessions"),
        PathBuf::from("sessions"),
    ];

    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for base in base_dirs {
        for name in &file_names {
            let p = base.join(name);
            if seen.insert(p.clone()) {
                candidates.push(p);
            }
        }
    }
    candidates
}

fn build_metadata(item: &Value) -> Option<Value> {
    let obj = item.as_object()?;
    Some(serde_json::json!({
        "original_id": obj.get("id").cloned(),
        "original_name": obj.get("name").cloned(),
        "metadata": obj.get("metadata").cloned(),
    }))
}

fn message_role(item: &Value) -> String {
    item.get("role")
        .and_then(Value::as_str)
        .or_else(|| item.get("name").and_then(Value::as_str))
        .unwrap_or("assistant")
        .to_string()
}

fn message_name(item: &Value) -> Option<String> {
    item.get("name")
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string)
}

fn new_message(
    role: &str,
    message_type: &str,
    content: Vec<Value>,
    name: Option<String>,
    metadata: Option<Value>,
) -> ChatMessage {
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: role.to_string(),
        message_type: message_type.to_string(),
        content,
        metadata,
        name,
    }
}

fn block_string_or_json(block: &Value, key: &str) -> Option<String> {
    let val = block.get(key)?;
    if let Some(s) = val.as_str() {
        return Some(s.to_string());
    }
    serde_json::to_string(val).ok()
}

fn flush_current(
    out: &mut Vec<ChatMessage>,
    role: &str,
    name: &Option<String>,
    metadata: &Option<Value>,
    current_type: &mut Option<&'static str>,
    current_content: &mut Vec<Value>,
) {
    if let Some(msg_type) = current_type.take() {
        if !current_content.is_empty() {
            out.push(new_message(
                role,
                msg_type,
                std::mem::take(current_content),
                name.clone(),
                metadata.clone(),
            ));
        }
    }
}

fn ensure_current_type(
    out: &mut Vec<ChatMessage>,
    role: &str,
    name: &Option<String>,
    metadata: &Option<Value>,
    current_type: &mut Option<&'static str>,
    current_content: &mut Vec<Value>,
    wanted: &'static str,
) {
    if *current_type != Some(wanted) {
        flush_current(out, role, name, metadata, current_type, current_content);
        *current_type = Some(wanted);
    }
}

fn memory_item_to_chat_messages(item: &Value) -> Vec<ChatMessage> {
    let mut out = Vec::new();
    let role = message_role(item);
    let name = message_name(item);
    let metadata = build_metadata(item);

    let content = item.get("content");
    let Some(content) = content else {
        return out;
    };

    if let Some(s) = content.as_str() {
        if !s.trim().is_empty() {
            out.push(new_message(
                &role,
                "message",
                vec![serde_json::json!({"type":"text","text": s})],
                name,
                metadata,
            ));
        }
        return out;
    }

    let Some(blocks) = content.as_array() else {
        return out;
    };

    let mut current_type: Option<&'static str> = None;
    let mut current_content: Vec<Value> = Vec::new();

    for block in blocks {
        let Some(obj) = block.as_object() else {
            continue;
        };
        let btype = obj.get("type").and_then(Value::as_str).unwrap_or("text");
        match btype {
            "text" => {
                let text = obj.get("text").and_then(Value::as_str).unwrap_or("");
                if text.trim().is_empty() {
                    continue;
                }
                ensure_current_type(
                    &mut out,
                    &role,
                    &name,
                    &metadata,
                    &mut current_type,
                    &mut current_content,
                    "message",
                );
                current_content.push(serde_json::json!({"type":"text","text": text}));
            }
            "thinking" => {
                let text = obj.get("thinking").and_then(Value::as_str).unwrap_or("");
                if text.trim().is_empty() {
                    continue;
                }
                ensure_current_type(
                    &mut out,
                    &role,
                    &name,
                    &metadata,
                    &mut current_type,
                    &mut current_content,
                    "reasoning",
                );
                current_content.push(serde_json::json!({"type":"text","text": text}));
            }
            "tool_use" => {
                flush_current(
                    &mut out,
                    &role,
                    &name,
                    &metadata,
                    &mut current_type,
                    &mut current_content,
                );
                let arguments = block_string_or_json(block, "input").unwrap_or_default();
                out.push(new_message(
                    &role,
                    "plugin_call",
                    vec![serde_json::json!({
                        "type":"data",
                        "data":{
                            "call_id": obj.get("id").cloned().unwrap_or(Value::Null),
                            "name": obj.get("name").cloned().unwrap_or(Value::Null),
                            "arguments": arguments
                        }
                    })],
                    name.clone(),
                    metadata.clone(),
                ));
            }
            "tool_result" => {
                flush_current(
                    &mut out,
                    &role,
                    &name,
                    &metadata,
                    &mut current_type,
                    &mut current_content,
                );
                let output = block_string_or_json(block, "output").unwrap_or_default();
                out.push(new_message(
                    &role,
                    "plugin_call_output",
                    vec![serde_json::json!({
                        "type":"data",
                        "data":{
                            "call_id": obj.get("id").cloned().unwrap_or(Value::Null),
                            "name": obj.get("name").cloned().unwrap_or(Value::Null),
                            "output": output
                        }
                    })],
                    name.clone(),
                    metadata.clone(),
                ));
            }
            "image" => {
                let image_url = obj
                    .get("source")
                    .and_then(Value::as_object)
                    .and_then(|src| src.get("url"))
                    .and_then(Value::as_str)
                    .or_else(|| obj.get("url").and_then(Value::as_str))
                    .unwrap_or("");
                if image_url.is_empty() {
                    continue;
                }
                ensure_current_type(
                    &mut out,
                    &role,
                    &name,
                    &metadata,
                    &mut current_type,
                    &mut current_content,
                    "message",
                );
                current_content.push(serde_json::json!({"type":"image","image_url": image_url}));
            }
            _ => {}
        }
    }

    flush_current(
        &mut out,
        &role,
        &name,
        &metadata,
        &mut current_type,
        &mut current_content,
    );

    out
}

#[cfg(test)]
fn memory_item_to_chat_message(item: &Value) -> Option<ChatMessage> {
    let mut msgs = memory_item_to_chat_messages(item);
    if msgs.is_empty() {
        None
    } else {
        Some(msgs.remove(0))
    }
}

fn legacy_memory_item_to_chat_message(item: &Value) -> Option<ChatMessage> {
    let obj = item.as_object()?;
    let role = obj
        .get("role")
        .and_then(Value::as_str)
        .or_else(|| obj.get("name").and_then(Value::as_str))
        .unwrap_or("assistant")
        .to_string();
    let content = item.get("content")?.as_str()?.to_string();
    let name = obj
        .get("name")
        .and_then(Value::as_str)
        .map(std::string::ToString::to_string);
    Some(ChatMessage {
        id: Uuid::new_v4().to_string(),
        role,
        message_type: "message".to_string(),
        content: vec![serde_json::json!({"type":"text","text": content})],
        metadata: None,
        name,
    })
}

async fn load_session_messages(session_id: &str, user_id: &str) -> Vec<ChatMessage> {
    for candidate in build_session_file_candidates(session_id, user_id) {
        let content = match tokio::fs::read_to_string(&candidate).await {
            Ok(c) => c,
            Err(_) => continue,
        };
        let state: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(memories) = state
            .get("agent")
            .and_then(|v| v.get("memory"))
            .and_then(Value::as_array)
        else {
            continue;
        };

        let mut out = Vec::new();
        for item in memories {
            let msgs = memory_item_to_chat_messages(item);
            if msgs.is_empty() {
                if let Some(msg) = legacy_memory_item_to_chat_message(item) {
                    out.push(msg);
                }
                continue;
            }
            out.extend(msgs);
        }
        return out;
    }
    Vec::new()
}

/// PUT /api/chats/:chat_id - Update a chat.
pub async fn update_chat(
    State(state): State<ChatsState>,
    Path(chat_id): Path<String>,
    Json(req): Json<UpdateChatRequest>,
) -> Result<Json<ChatSpec>, ChatRouteError> {
    let chat = state.manager.update(&chat_id, req).await?;
    Ok(Json(chat))
}

/// DELETE /api/chats/:chat_id - Delete a chat.
pub async fn delete_chat(
    State(state): State<ChatsState>,
    Path(chat_id): Path<String>,
) -> Result<Json<Value>, ChatRouteError> {
    state.manager.delete(&chat_id).await?;
    Ok(Json(
        serde_json::json!({ "success": true, "chat_id": chat_id, "deleted": true }),
    ))
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub(crate) enum BatchDeletePayload {
    Object(BatchDeleteRequest),
    Array(Vec<String>),
}

/// Create the chats router.
pub fn create_chats_router() -> axum::Router<ChatsState> {
    use axum::routing::*;

    axum::Router::new()
        .route("/", get(list_chats).post(create_chat))
        .route("/batch-delete", post(batch_delete_chats))
        .route(
            "/:chat_id",
            get(get_chat).put(update_chat).delete(delete_chat),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::chat_repo::ChatRepository;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use uuid::Uuid;

    async fn create_test_state() -> (ChatsState, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("chats.json");
        let repo = ChatRepository::with_path(repo_path).unwrap();
        let manager = ChatManager::with_repository(repo);
        let state = ChatsState {
            manager: Arc::new(manager),
        };
        (state, temp_dir)
    }

    #[tokio::test]
    async fn test_list_chats_empty() {
        let (state, _temp) = create_test_state().await;
        let query = ChatListQuery {
            channel: None,
            user_id: None,
            session_id: None,
        };

        let result = list_chats(State(state), Query(query)).await;
        assert!(result.is_ok());
        let chats = result.unwrap().0;
        assert!(chats.is_empty());
    }

    #[tokio::test]
    async fn test_create_and_list_chats() {
        let (state, _temp) = create_test_state().await;

        let req = CreateChatRequest {
            name: "Test Chat".to_string(),
            session_id: "console:test-user".to_string(),
            user_id: "test-user".to_string(),
            channel: "console".to_string(),
        };

        let chat = create_chat(State(state.clone()), Json(req))
            .await
            .unwrap()
            .0;

        assert!(!chat.id.is_empty());
        assert_eq!(chat.name, "Test Chat");

        // List all chats
        let query = ChatListQuery {
            channel: None,
            user_id: None,
            session_id: None,
        };

        let result = list_chats(State(state), Query(query)).await;
        assert!(result.is_ok());
        let chats = result.unwrap().0;
        assert_eq!(chats.len(), 1);
    }

    #[tokio::test]
    async fn test_get_chat() {
        let (state, _temp) = create_test_state().await;

        let req = CreateChatRequest {
            name: "Test Chat".to_string(),
            session_id: "console:test-user".to_string(),
            user_id: "test-user".to_string(),
            channel: "console".to_string(),
        };

        let chat = create_chat(State(state.clone()), Json(req))
            .await
            .unwrap()
            .0;

        let result = get_chat(State(state), Path(chat.id)).await;
        assert!(result.is_ok());
        let history = result.unwrap().0;
        assert!(history.messages.is_empty());
    }

    #[test]
    fn test_memory_item_to_chat_message() {
        let item = serde_json::json!({
            "name": "assistant",
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": "step1"},
                {"type": "text", "text": "hello world"}
            ]
        });
        let msg = memory_item_to_chat_message(&item).expect("message should parse");
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.message_type, "reasoning");
        assert_eq!(msg.content[0]["type"], "text");
        assert_eq!(msg.content[0]["text"], "step1");

        let all = memory_item_to_chat_messages(&item);
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].message_type, "message");
        assert_eq!(all[1].content[0]["text"], "hello world");
    }

    #[tokio::test]
    async fn test_update_chat() {
        let (state, _temp) = create_test_state().await;

        let req = CreateChatRequest {
            name: "Original Name".to_string(),
            session_id: "console:test-user".to_string(),
            user_id: "test-user".to_string(),
            channel: "console".to_string(),
        };

        let chat = create_chat(State(state.clone()), Json(req))
            .await
            .unwrap()
            .0;

        let update_req = UpdateChatRequest {
            name: Some("Updated Name".to_string()),
            meta: std::collections::HashMap::new(),
        };

        let result = update_chat(State(state), Path(chat.id), Json(update_req)).await;
        assert!(result.is_ok());
        let updated = result.unwrap().0;
        assert_eq!(updated.name, "Updated Name");
    }

    #[tokio::test]
    async fn test_delete_chat() {
        let (state, _temp) = create_test_state().await;

        let req = CreateChatRequest {
            name: "Test Chat".to_string(),
            session_id: "console:test-user".to_string(),
            user_id: "test-user".to_string(),
            channel: "console".to_string(),
        };

        let chat = create_chat(State(state.clone()), Json(req))
            .await
            .unwrap()
            .0;

        let result = delete_chat(State(state), Path(chat.id)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_batch_delete() {
        let (state, _temp) = create_test_state().await;

        let req1 = CreateChatRequest {
            name: "Chat 1".to_string(),
            session_id: "s1".to_string(),
            user_id: "user1".to_string(),
            channel: "console".to_string(),
        };

        let req2 = CreateChatRequest {
            name: "Chat 2".to_string(),
            session_id: "s2".to_string(),
            user_id: "user2".to_string(),
            channel: "console".to_string(),
        };

        let chat1 = create_chat(State(state.clone()), Json(req1.clone()))
            .await
            .unwrap()
            .0;
        let chat2 = create_chat(State(state.clone()), Json(req2.clone()))
            .await
            .unwrap()
            .0;

        let batch_req = BatchDeleteRequest {
            ids: vec![chat1.id, chat2.id],
        };

        let result =
            batch_delete_chats(State(state), Json(BatchDeletePayload::Object(batch_req))).await;
        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response["deleted_count"], 2);
        assert_eq!(response["success"], true);
    }

    #[tokio::test]
    async fn test_error_not_found() {
        let (state, _temp) = create_test_state().await;
        let result = get_chat(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_session_messages_from_python_style_session_file() {
        let sid = format!("console:test-{}", Uuid::new_v4());
        let uid = format!("user-{}", Uuid::new_v4());
        let safe_sid = sanitize_filename(&sid);
        let safe_uid = sanitize_filename(&uid);

        let sessions_dir = PathBuf::from("sessions");
        tokio::fs::create_dir_all(&sessions_dir).await.unwrap();
        let file_path = sessions_dir.join(format!("{safe_uid}_{safe_sid}.json"));

        let payload = serde_json::json!({
            "agent": {
                "memory": [
                    {
                        "name": "user",
                        "role": "user",
                        "content": [{"type":"text","text":"hello"}]
                    },
                    {
                        "name": "assistant",
                        "role": "assistant",
                        "content": [{"type":"text","text":"world"}]
                    }
                ]
            }
        });
        tokio::fs::write(&file_path, serde_json::to_string_pretty(&payload).unwrap())
            .await
            .unwrap();

        let messages = load_session_messages(&sid, &uid).await;
        assert!(!messages.is_empty());
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].message_type, "message");

        let _ = tokio::fs::remove_file(&file_path).await;
    }
}
