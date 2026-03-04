//! Chat API routes.
//!
//! Provides REST endpoints for chat management.

use crate::runner::chat_manager::{ChatManager, ChatManagerError};
use crate::routes::schemas::{
    BatchDeleteRequest, ChatHistory, ChatListQuery, ChatMessage, ChatSpec, CreateChatRequest,
    UpdateChatRequest,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::Value;
use std::sync::Arc;

/// State for chat routes.
#[derive(Clone)]
pub struct ChatsState {
    pub manager: Arc<ChatManager>,
}

/// Trait for accessing chats state.
pub trait HasChatsState {
    fn chats_state(&self) -> &ChatsState;
}

impl HasChatsState for ChatsState {
    fn chats_state(&self) -> &ChatsState {
        self
    }
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
    Json(req): Json<BatchDeleteRequest>,
) -> Result<Json<Value>, ChatRouteError> {
    let deleted = state.manager.delete_batch(req.ids).await?;
    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

/// GET /api/chats/:chat_id - Get chat history.
pub async fn get_chat(
    State(state): State<ChatsState>,
    Path(chat_id): Path<String>,
) -> Result<Json<ChatHistory>, ChatRouteError> {
    let _chat = state.manager.get(&chat_id).await?;

    // For now, return empty history
    // TODO: Implement actual message loading from chat files
    let history = ChatHistory {
        messages: Vec::new(),
    };

    Ok(Json(history))
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
    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// Create the chats router.
pub fn create_chats_router() -> axum::Router<ChatsState> {
    use axum::routing::*;

    axum::Router::new()
        .route("/", get(list_chats).post(create_chat))
        .route("/batch-delete", post(batch_delete_chats))
        .route("/:chat_id", get(get_chat).put(update_chat).delete(delete_chat))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::repo::chat_repo::ChatRepository;

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

        let mut req1 = CreateChatRequest {
            name: "Chat 1".to_string(),
            session_id: "s1".to_string(),
            user_id: "user1".to_string(),
            channel: "console".to_string(),
        };

        let mut req2 = CreateChatRequest {
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

        let result = batch_delete_chats(State(state), Json(batch_req)).await;
        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response["deleted"], 2);
    }

    #[tokio::test]
    async fn test_error_not_found() {
        let (state, _temp) = create_test_state().await;
        let result = get_chat(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }
}
