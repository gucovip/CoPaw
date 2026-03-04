//! Chat manager for handling chat operations.
//!
//! Provides business logic for chat CRUD operations with filtering.

use crate::repo::chat_repo::{ChatRepository, ChatRepositoryError};
use crate::routes::schemas::{ChatSpec, CreateChatRequest, UpdateChatRequest};
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur in chat manager operations.
#[derive(Debug, Error)]
pub enum ChatManagerError {
    #[error("Repository error: {0}")]
    Repository(#[from] ChatRepositoryError),

    #[error("Chat not found: {0}")]
    ChatNotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Chat manager with business logic for chat operations.
#[derive(Clone)]
pub struct ChatManager {
    repo: Arc<ChatRepository>,
}

impl ChatManager {
    /// Create a new chat manager with the default repository.
    pub fn new() -> Result<Self, ChatManagerError> {
        let repo = Arc::new(ChatRepository::new()?);
        Ok(Self { repo })
    }

    /// Create a new chat manager with a custom repository.
    pub fn with_repository(repo: ChatRepository) -> Self {
        Self {
            repo: Arc::new(repo),
        }
    }

    /// List all chats, optionally filtered.
    pub async fn list(
        &self,
        channel: Option<&str>,
        user_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Vec<ChatSpec>, ChatManagerError> {
        let mut chats = self.repo.list().await?;

        // Apply filters
        if let Some(channel) = channel {
            chats.retain(|c| c.channel == channel);
        }
        if let Some(user_id) = user_id {
            chats.retain(|c| c.user_id == user_id);
        }
        if let Some(session_id) = session_id {
            chats.retain(|c| c.session_id == session_id);
        }

        Ok(chats)
    }

    /// Get a chat by ID.
    pub async fn get(&self, id: &str) -> Result<ChatSpec, ChatManagerError> {
        self.repo.get(id).await.map_err(|e| match e {
            ChatRepositoryError::ChatNotFound(id) => ChatManagerError::ChatNotFound(id),
            _ => ChatManagerError::Repository(e),
        })
    }

    /// Create a new chat.
    pub async fn create(&self, req: CreateChatRequest) -> Result<ChatSpec, ChatManagerError> {
        if req.session_id.is_empty() {
            return Err(ChatManagerError::InvalidInput("session_id is required".to_string()));
        }
        if req.user_id.is_empty() {
            return Err(ChatManagerError::InvalidInput("user_id is required".to_string()));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let chat = ChatSpec {
            id: uuid::Uuid::new_v4().to_string(),
            name: req.name,
            session_id: req.session_id,
            user_id: req.user_id,
            channel: req.channel,
            created_at: now.clone(),
            updated_at: now,
            meta: std::collections::HashMap::new(),
        };

        self.repo.add(chat.clone()).await?;
        Ok(chat)
    }

    /// Update an existing chat.
    pub async fn update(
        &self,
        id: &str,
        req: UpdateChatRequest,
    ) -> Result<ChatSpec, ChatManagerError> {
        let existing = self.repo.get(id).await.map_err(|e| match e {
            ChatRepositoryError::ChatNotFound(id) => ChatManagerError::ChatNotFound(id),
            _ => ChatManagerError::Repository(e),
        })?;

        let updated_chat = ChatSpec {
            name: req.name.unwrap_or(existing.name),
            meta: if req.meta.is_empty() {
                existing.meta
            } else {
                req.meta
            },
            ..existing
        };

        self.repo.update(id, updated_chat.clone()).await?;
        Ok(updated_chat)
    }

    /// Delete a chat by ID.
    pub async fn delete(&self, id: &str) -> Result<(), ChatManagerError> {
        self.repo.delete(id).await.map_err(|e| match e {
            ChatRepositoryError::ChatNotFound(id) => ChatManagerError::ChatNotFound(id),
            _ => ChatManagerError::Repository(e),
        })
    }

    /// Delete multiple chats by ID.
    pub async fn delete_batch(&self, ids: Vec<String>) -> Result<usize, ChatManagerError> {
        if ids.is_empty() {
            return Ok(0);
        }
        self.repo.delete_batch(&ids).await.map_err(ChatManagerError::from)
    }

    /// Find a chat by session ID.
    pub async fn find_by_session(&self, session_id: &str) -> Result<Option<ChatSpec>, ChatManagerError> {
        self.repo.find_by_session(session_id).await.map_err(ChatManagerError::from)
    }
}

impl Default for ChatManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default chat manager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_manager() -> (ChatManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("chats.json");
        let repo = ChatRepository::with_path(repo_path).unwrap();
        let manager = ChatManager::with_repository(repo);
        (manager, temp_dir)
    }

    fn create_test_request() -> CreateChatRequest {
        CreateChatRequest {
            name: "Test Chat".to_string(),
            session_id: "console:test-user".to_string(),
            user_id: "test-user".to_string(),
            channel: "console".to_string(),
        }
    }

    #[tokio::test]
    async fn test_manager_create_chat() {
        let (manager, _temp) = create_test_manager().await;
        let req = create_test_request();

        let chat = manager.create(req).await.unwrap();
        assert!(!chat.id.is_empty());
        assert_eq!(chat.name, "Test Chat");
        assert_eq!(chat.session_id, "console:test-user");
    }

    #[tokio::test]
    async fn test_manager_create_validation() {
        let (manager, _temp) = create_test_manager().await;
        let req = CreateChatRequest {
            name: "Test".to_string(),
            session_id: "".to_string(), // Empty session_id
            user_id: "test".to_string(),
            channel: "console".to_string(),
        };

        let result = manager.create(req).await;
        assert!(matches!(result, Err(ChatManagerError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_manager_get_chat() {
        let (manager, _temp) = create_test_manager().await;
        let req = create_test_request();
        let chat = manager.create(req).await.unwrap();

        let retrieved = manager.get(&chat.id).await.unwrap();
        assert_eq!(retrieved.id, chat.id);
    }

    #[tokio::test]
    async fn test_manager_get_not_found() {
        let (manager, _temp) = create_test_manager().await;
        let result = manager.get("nonexistent").await;
        assert!(matches!(result, Err(ChatManagerError::ChatNotFound(_))));
    }

    #[tokio::test]
    async fn test_manager_list_chats() {
        let (manager, _temp) = create_test_manager().await;

        let mut req1 = create_test_request();
        req1.session_id = "console:user1".to_string();

        let mut req2 = create_test_request();
        req2.session_id = "console:user2".to_string();
        req2.channel = "discord".to_string();

        manager.create(req1).await.unwrap();
        manager.create(req2).await.unwrap();

        let all = manager.list(None, None, None).await.unwrap();
        assert_eq!(all.len(), 2);

        let console_only = manager.list(Some("console"), None, None).await.unwrap();
        assert_eq!(console_only.len(), 1);
    }

    #[tokio::test]
    async fn test_manager_update_chat() {
        let (manager, _temp) = create_test_manager().await;
        let req = create_test_request();
        let chat = manager.create(req).await.unwrap();

        let update_req = UpdateChatRequest {
            name: Some("Updated Chat".to_string()),
            meta: std::collections::HashMap::new(),
        };

        let updated = manager.update(&chat.id, update_req).await.unwrap();
        assert_eq!(updated.name, "Updated Chat");
    }

    #[tokio::test]
    async fn test_manager_delete_chat() {
        let (manager, _temp) = create_test_manager().await;
        let req = create_test_request();
        let chat = manager.create(req).await.unwrap();

        manager.delete(&chat.id).await.unwrap();

        let result = manager.get(&chat.id).await;
        assert!(matches!(result, Err(ChatManagerError::ChatNotFound(_))));
    }

    #[tokio::test]
    async fn test_manager_delete_batch() {
        let (manager, _temp) = create_test_manager().await;

        let mut req1 = create_test_request();
        let mut req2 = create_test_request();
        let mut req3 = create_test_request();

        req1.session_id = "s1".to_string();
        req2.session_id = "s2".to_string();
        req3.session_id = "s3".to_string();

        let chat1 = manager.create(req1).await.unwrap();
        let chat2 = manager.create(req2).await.unwrap();
        let chat3 = manager.create(req3).await.unwrap();

        let deleted = manager
            .delete_batch(vec![chat1.id.clone(), chat3.id.clone()])
            .await
            .unwrap();
        assert_eq!(deleted, 2);

        let remaining = manager.list(None, None, None).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, chat2.id);
    }

    #[tokio::test]
    async fn test_manager_find_by_session() {
        let (manager, _temp) = create_test_manager().await;
        let req = create_test_request();
        let chat = manager.create(req).await.unwrap();

        let found = manager.find_by_session(&chat.session_id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, chat.id);
    }
}
