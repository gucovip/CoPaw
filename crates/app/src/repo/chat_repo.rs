//! JSON-based chat repository.
//!
//! Provides persistent storage for chat metadata in ~/.copaw/chats/chats.json.
//! Similar to the Python JsonChatRepository.

use crate::routes::schemas::{ChatSpec, ChatsFile};
use copaw_config::get_chats_path;
use serde_json;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur in chat repository operations.
#[derive(Debug, Error)]
pub enum ChatRepositoryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Chat not found: {0}")]
    ChatNotFound(String),

    #[error("Invalid chat data: {0}")]
    InvalidData(String),
}

/// JSON-based chat repository for storing chat metadata.
///
/// Stores chat specs in a single JSON file at ~/.copaw/chats/chats.json.
/// Uses atomic writes (write to temp file, then replace) for data safety.
#[derive(Clone)]
pub struct ChatRepository {
    /// Path to the chats.json file
    path: PathBuf,
}

impl ChatRepository {
    /// Create a new chat repository with the default path.
    ///
    /// The default path is ~/.copaw/chats/chats.json
    pub fn new() -> Result<Self, ChatRepositoryError> {
        let path = get_chats_path().join("chats.json");
        Self::with_path(path)
    }

    /// Create a new chat repository with a custom path.
    pub fn with_path<P: AsRef<Path>>(path: P) -> Result<Self, ChatRepositoryError> {
        let path = path.as_ref().to_path_buf();
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self { path })
    }

    /// Get the repository file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load all chat specs from the repository.
    ///
    /// Returns an empty ChatsFile if the file doesn't exist yet.
    pub async fn load(&self) -> Result<ChatsFile, ChatRepositoryError> {
        if !self.path.exists() {
            return Ok(ChatsFile::default());
        }

        let content = tokio::fs::read_to_string(&self.path).await?;
        let chats_file: ChatsFile = serde_json::from_str(&content)?;
        Ok(chats_file)
    }

    /// Save all chat specs to the repository.
    ///
    /// Uses atomic write: writes to a temp file first, then replaces
    /// the original file.
    pub async fn save(&self, chats_file: &ChatsFile) -> Result<(), ChatRepositoryError> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Serialize to JSON
        let payload = serde_json::to_string_pretty(chats_file)?;

        // Write to temp file first
        let tmp_path = self.path.with_extension("json.tmp");
        tokio::fs::write(&tmp_path, payload).await?;

        // Atomic replace
        tokio::fs::rename(&tmp_path, &self.path).await?;

        Ok(())
    }

    /// List all chat specs.
    pub async fn list(&self) -> Result<Vec<ChatSpec>, ChatRepositoryError> {
        let chats_file = self.load().await?;
        Ok(chats_file.chats)
    }

    /// Get a chat spec by ID.
    pub async fn get(&self, id: &str) -> Result<ChatSpec, ChatRepositoryError> {
        let chats_file = self.load().await?;
        chats_file
            .chats
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| ChatRepositoryError::ChatNotFound(id.to_string()))
    }

    /// Add a new chat spec.
    pub async fn add(&self, chat: ChatSpec) -> Result<(), ChatRepositoryError> {
        let mut chats_file = self.load().await?;
        chats_file.chats.push(chat);
        self.save(&chats_file).await
    }

    /// Update an existing chat spec.
    pub async fn update(&self, id: &str, mut updated_chat: ChatSpec) -> Result<(), ChatRepositoryError> {
        let mut chats_file = self.load().await?;
        let chat = chats_file
            .chats
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| ChatRepositoryError::ChatNotFound(id.to_string()))?;

        // Preserve the ID and timestamps
        updated_chat.id = chat.id.clone();
        updated_chat.created_at = chat.created_at.clone();
        updated_chat.updated_at = chrono::Utc::now().to_rfc3339();

        *chat = updated_chat;
        self.save(&chats_file).await
    }

    /// Delete a chat spec by ID.
    pub async fn delete(&self, id: &str) -> Result<(), ChatRepositoryError> {
        let mut chats_file = self.load().await?;
        let original_len = chats_file.chats.len();
        chats_file.chats.retain(|c| c.id != id);

        if chats_file.chats.len() == original_len {
            return Err(ChatRepositoryError::ChatNotFound(id.to_string()));
        }

        self.save(&chats_file).await
    }

    /// Delete multiple chat specs by ID.
    pub async fn delete_batch(&self, ids: &[String]) -> Result<usize, ChatRepositoryError> {
        let mut chats_file = self.load().await?;
        let original_len = chats_file.chats.len();
        let ids_set: std::collections::HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();

        chats_file.chats.retain(|c| !ids_set.contains(c.id.as_str()));
        let deleted_count = original_len - chats_file.chats.len();

        if deleted_count > 0 {
            self.save(&chats_file).await?;
        }

        Ok(deleted_count)
    }

    /// Find a chat spec by session ID.
    pub async fn find_by_session(&self, session_id: &str) -> Result<Option<ChatSpec>, ChatRepositoryError> {
        let chats_file = self.load().await?;
        Ok(chats_file.chats.into_iter().find(|c| c.session_id == session_id))
    }
}

impl Default for ChatRepository {
    fn default() -> Self {
        Self::new().expect("Failed to create default chat repository")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_repo() -> (ChatRepository, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("chats.json");
        let repo = ChatRepository::with_path(repo_path).unwrap();
        (repo, temp_dir)
    }

    fn create_test_chat() -> ChatSpec {
        ChatSpec {
            id: "test-chat-1".to_string(),
            name: "Test Chat".to_string(),
            session_id: "console:test-user".to_string(),
            user_id: "test-user".to_string(),
            channel: "console".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            meta: std::collections::HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_repo_create() {
        let (repo, _temp) = create_test_repo().await;
        assert!(!repo.path().exists());
    }

    #[tokio::test]
    async fn test_repo_load_empty() {
        let (repo, _temp) = create_test_repo().await;
        let chats = repo.load().await.unwrap();
        assert_eq!(chats.version, 1);
        assert!(chats.chats.is_empty());
    }

    #[tokio::test]
    async fn test_repo_add_chat() {
        let (repo, _temp) = create_test_repo().await;
        let chat = create_test_chat();

        repo.add(chat.clone()).await.unwrap();

        let chats = repo.load().await.unwrap();
        assert_eq!(chats.chats.len(), 1);
        assert_eq!(chats.chats[0].id, chat.id);
        assert_eq!(chats.chats[0].name, chat.name);
    }

    #[tokio::test]
    async fn test_repo_list_chats() {
        let (repo, _temp) = create_test_repo().await;
        let chat1 = create_test_chat();
        let mut chat2 = create_test_chat();
        chat2.id = "test-chat-2".to_string();
        chat2.session_id = "console:test-user-2".to_string();

        repo.add(chat1).await.unwrap();
        repo.add(chat2).await.unwrap();

        let chats = repo.list().await.unwrap();
        assert_eq!(chats.len(), 2);
    }

    #[tokio::test]
    async fn test_repo_get_chat() {
        let (repo, _temp) = create_test_repo().await;
        let chat = create_test_chat();
        repo.add(chat.clone()).await.unwrap();

        let retrieved = repo.get(&chat.id).await.unwrap();
        assert_eq!(retrieved.id, chat.id);
        assert_eq!(retrieved.name, chat.name);
    }

    #[tokio::test]
    async fn test_repo_get_not_found() {
        let (repo, _temp) = create_test_repo().await;
        let result = repo.get("nonexistent").await;
        assert!(matches!(result, Err(ChatRepositoryError::ChatNotFound(_))));
    }

    #[tokio::test]
    async fn test_repo_update_chat() {
        let (repo, _temp) = create_test_repo().await;
        let mut chat = create_test_chat();
        repo.add(chat.clone()).await.unwrap();

        // Update the chat
        chat.name = "Updated Chat".to_string();
        let chat_id = chat.id.clone();
        repo.update(&chat_id, chat).await.unwrap();

        let retrieved = repo.get(&chat_id).await.unwrap();
        assert_eq!(retrieved.name, "Updated Chat");
        // ID and created_at should be preserved
        assert_eq!(retrieved.id, "test-chat-1");
    }

    #[tokio::test]
    async fn test_repo_delete_chat() {
        let (repo, _temp) = create_test_repo().await;
        let chat = create_test_chat();
        repo.add(chat.clone()).await.unwrap();

        repo.delete(&chat.id).await.unwrap();

        let result = repo.get(&chat.id).await;
        assert!(matches!(result, Err(ChatRepositoryError::ChatNotFound(_))));
    }

    #[tokio::test]
    async fn test_repo_delete_batch() {
        let (repo, _temp) = create_test_repo().await;
        let mut chat1 = create_test_chat();
        let mut chat2 = create_test_chat();
        let mut chat3 = create_test_chat();

        chat2.id = "test-chat-2".to_string();
        chat3.id = "test-chat-3".to_string();

        repo.add(chat1.clone()).await.unwrap();
        repo.add(chat2.clone()).await.unwrap();
        repo.add(chat3.clone()).await.unwrap();

        let ids = vec![chat1.id.clone(), chat3.id.clone()];
        let deleted_count = repo.delete_batch(&ids).await.unwrap();
        assert_eq!(deleted_count, 2);

        let chats = repo.list().await.unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].id, chat2.id);
    }

    #[tokio::test]
    async fn test_repo_find_by_session() {
        let (repo, _temp) = create_test_repo().await;
        let chat = create_test_chat();
        repo.add(chat.clone()).await.unwrap();

        let found = repo.find_by_session(&chat.session_id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, chat.id);
    }

    #[tokio::test]
    async fn test_repo_persistence() {
        let (repo, temp) = create_test_repo().await;
        let chat = create_test_chat();
        repo.add(chat.clone()).await.unwrap();

        // Create a new repo instance with the same path
        let repo2 = ChatRepository::with_path(temp.path().join("chats.json")).unwrap();
        let retrieved = repo2.get(&chat.id).await.unwrap();
        assert_eq!(retrieved.id, chat.id);
    }
}
