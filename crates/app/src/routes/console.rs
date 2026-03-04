// -*- coding: utf-8 -*-
// API routes for console push messages.

use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Console push message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushMessage {
    pub id: String,
    pub text: String,
}

/// Push messages response.
#[derive(Debug, Clone, Serialize)]
pub struct PushMessagesResponse {
    pub messages: Vec<PushMessage>,
}

/// Push messages query parameters.
#[derive(Debug, Deserialize)]
pub struct PushMessagesQuery {
    pub session_id: Option<String>,
}

/// In-memory store for console push messages.
struct PushMessageStore {
    messages: Arc<RwLock<Vec<StoredMessage>>>,
}

/// Internal stored message with timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredMessage {
    id: String,
    text: String,
    ts: f64,
    session_id: String,
}

impl PushMessageStore {
    const MAX_AGE_SECONDS: f64 = 60.0;
    const MAX_MESSAGES: usize = 500;

    fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Append a message to the store.
    async fn append(&self, session_id: &str, text: &str) {
        if session_id.is_empty() || text.is_empty() {
            return;
        }

        let mut messages = self.messages.write().await;
        messages.push(StoredMessage {
            id: Uuid::new_v4().to_string(),
            text: text.to_string(),
            ts: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
            session_id: session_id.to_string(),
        });

        // Drop oldest if over max messages
        if messages.len() > Self::MAX_MESSAGES {
            messages.sort_by(|a, b| a.ts.partial_cmp(&b.ts).unwrap_or(std::cmp::Ordering::Equal));
            let drop_count = messages.len() - Self::MAX_MESSAGES;
            messages.drain(0..drop_count);
        }
    }

    /// Take and remove all messages for a session.
    async fn take(&self, session_id: &str) -> Vec<StoredMessage> {
        if session_id.is_empty() {
            return Vec::new();
        }

        let mut messages_guard = self.messages.write().await;
        let out: Vec<StoredMessage> = messages_guard
            .iter()
            .filter(|m| m.session_id == session_id)
            .cloned()
            .collect();
        // Remove the taken messages
        messages_guard.retain(|m| m.session_id != session_id);
        out
    }

    /// Get recent messages (not consumed, last MAX_AGE_SECONDS).
    async fn get_recent(&self) -> Vec<StoredMessage> {
        let now = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
        let cutoff = now - Self::MAX_AGE_SECONDS;

        let mut messages = self.messages.write().await;
        // Drop old messages
        messages.retain(|m| m.ts >= cutoff);
        messages.clone()
    }
}

lazy_static::lazy_static! {
    static ref PUSH_STORE: PushMessageStore = PushMessageStore::new();
}

/// GET /api/console/push-messages - Get pending push messages.
pub async fn get_push_messages(
    Query(query): Query<PushMessagesQuery>,
) -> Result<Json<PushMessagesResponse>, ConsoleError> {
    let stored: Vec<StoredMessage> = if let Some(session_id) = query.session_id {
        PUSH_STORE.take(&session_id).await
    } else {
        PUSH_STORE.get_recent().await
    };

    let messages: Vec<PushMessage> = stored
        .into_iter()
        .map(|m| PushMessage {
            id: m.id,
            text: m.text,
        })
        .collect();

    Ok(Json(PushMessagesResponse { messages }))
}

/// Console API error.
#[derive(Debug)]
pub enum ConsoleError {
    Internal(String),
}

impl IntoResponse for ConsoleError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ConsoleError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(serde_json::json!({ "detail": message }));
        (status, body).into_response()
    }
}

impl std::fmt::Display for ConsoleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsoleError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for ConsoleError {}

/// Create the console router.
pub fn create_console_router<S>() -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    use axum::routing::*;

    axum::Router::new().route("/push-messages", get(get_push_messages))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_push_messages_no_session() {
        let query = PushMessagesQuery { session_id: None };
        // This should work even with no messages
        let result = get_push_messages(Query(query)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_push_message_store() {
        let store = PushMessageStore::new();

        // Append a message
        store.append("test-session", "Test message").await;

        // Take messages for the session
        let taken = store.take("test-session").await;
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].text, "Test message");

        // Take again should return empty
        let taken = store.take("test-session").await;
        assert_eq!(taken.len(), 0);
    }
}
