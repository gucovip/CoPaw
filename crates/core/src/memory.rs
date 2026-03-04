// -*- coding: utf-8 -*-
// -*- mode: rust -*-
//! Memory trait and implementations for CoPaw agent.
//!
//! This module defines the memory abstractions used throughout the CoPaw system,
//! providing Rust equivalents to the Python AgentScope memory types.
//!
//! Based on the Python implementation:
//! - agentscope.memory.InMemoryMemory
//! - src/copaw/agents/memory/copaw_memory.py

use crate::message::Message;
use async_trait::async_trait;
use std::sync::{Arc, RwLock};

/// Errors that can occur when working with memory
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MemoryError {
    #[error("memory operation failed: {0}")]
    OperationFailed(String),

    #[error("memory is full")]
    MemoryFull,

    #[error("message not found: {0}")]
    MessageNotFound(String),

    #[error("invalid message: {0}")]
    InvalidMessage(String),
}

/// Memory trait for storing and retrieving messages
///
/// This trait defines the interface for memory implementations in the
/// CoPaw agent system. It provides methods for adding, retrieving, and
/// managing conversation messages.
///
/// Based on AgentScope's `InMemoryMemory` class and CoPaw's extensions.
#[async_trait]
pub trait Memory: Send + Sync {
    /// Add a message to memory
    ///
    /// # Errors
    ///
    /// Returns a `MemoryError` if the message cannot be added
    async fn add(&mut self, msg: Message) -> Result<(), MemoryError>;

    /// Get all messages from memory
    fn get_all(&self) -> Vec<Message>;

    /// Get the most recent n messages from memory
    ///
    /// If n is greater than the number of messages, returns all messages.
    fn get_recent(&self, n: usize) -> Vec<Message>;

    /// Clear all messages from memory
    ///
    /// # Errors
    ///
    /// Returns a `MemoryError` if the memory cannot be cleared
    async fn clear(&mut self) -> Result<(), MemoryError>;

    /// Get the total token count of all messages in memory
    ///
    /// This is an estimate based on the message content.
    fn token_count(&self) -> usize;
}

/// In-memory implementation of the Memory trait
///
/// Stores messages in an in-memory vector protected by a read-write lock.
/// This is the simplest memory implementation and is suitable for most
/// use cases.
///
/// Based on AgentScope's `InMemoryMemory` and CoPaw's `CoPawInMemoryMemory`.
#[derive(Debug, Clone)]
pub struct InMemoryMemory {
    /// Inner storage for messages
    inner: Arc<RwLock<Vec<Message>>>,
}

impl InMemoryMemory {
    /// Create a new empty in-memory storage
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the number of messages in memory
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().map(|v| v.len()).unwrap_or(0)
    }

    /// Check if memory is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for InMemoryMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Memory for InMemoryMemory {
    async fn add(&mut self, msg: Message) -> Result<(), MemoryError> {
        self.inner
            .write()
            .map(|mut v| v.push(msg))
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))
    }

    fn get_all(&self) -> Vec<Message> {
        self.inner.read().map(|v| v.clone()).unwrap_or_default()
    }

    fn get_recent(&self, n: usize) -> Vec<Message> {
        self.inner
            .read()
            .map(|v| {
                let len = v.len();
                let start = len.saturating_sub(n);
                v[start..].to_vec()
            })
            .unwrap_or_default()
    }

    async fn clear(&mut self) -> Result<(), MemoryError> {
        self.inner
            .write()
            .map(|mut v| v.clear())
            .map_err(|e| MemoryError::OperationFailed(e.to_string()))
    }

    fn token_count(&self) -> usize {
        self.inner
            .read()
            .map(|v| {
                v.iter()
                    .map(|msg| {
                        // Simple token estimation: 4 chars per token
                        let content_text = match msg.content() {
                            crate::message::MessageContent::Text(s) => s.len(),
                            crate::message::MessageContent::Parts(parts) => parts
                                .iter()
                                .map(|p| match p {
                                    crate::message::ContentPart::Text { text } => text.len(),
                                    _ => 100, // Estimate for non-text content
                                })
                                .sum(),
                        };
                        (content_text / 4).max(1)
                    })
                    .sum()
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;

    #[tokio::test]
    async fn test_in_memory_memory_add_and_get() {
        let mut memory = InMemoryMemory::new();

        memory.add(Message::user("hello")).await.unwrap();
        memory.add(Message::assistant("hi")).await.unwrap();

        assert_eq!(memory.get_all().len(), 2);
        assert!(!memory.is_empty());
        assert_eq!(memory.len(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_memory_get_recent() {
        let mut memory = InMemoryMemory::new();

        memory.add(Message::user("first")).await.unwrap();
        memory.add(Message::assistant("second")).await.unwrap();
        memory.add(Message::user("third")).await.unwrap();

        // Get most recent 1 message
        let recent = memory.get_recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].content().as_text().unwrap(), "third");

        // Get most recent 2 messages
        let recent = memory.get_recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].content().as_text().unwrap(), "second");
        assert_eq!(recent[1].content().as_text().unwrap(), "third");

        // Get more messages than exist
        let recent = memory.get_recent(10);
        assert_eq!(recent.len(), 3);
    }

    #[tokio::test]
    async fn test_in_memory_memory_clear() {
        let mut memory = InMemoryMemory::new();

        memory.add(Message::user("hello")).await.unwrap();
        assert_eq!(memory.len(), 1);

        memory.clear().await.unwrap();
        assert_eq!(memory.len(), 0);
        assert!(memory.is_empty());
        assert!(memory.get_all().is_empty());
    }

    #[tokio::test]
    async fn test_in_memory_memory_token_count() {
        let mut memory = InMemoryMemory::new();

        memory.add(Message::user("hello world")).await.unwrap();

        // "hello world" is 11 chars, roughly 3 tokens (11/4, min 1)
        let token_count = memory.token_count();
        assert!(token_count > 0);

        memory
            .add(Message::assistant("this is a much longer response"))
            .await
            .unwrap();

        let token_count2 = memory.token_count();
        assert!(token_count2 > token_count);
    }

    #[tokio::test]
    async fn test_in_memory_memory_empty() {
        let memory = InMemoryMemory::new();

        assert!(memory.is_empty());
        assert_eq!(memory.len(), 0);
        assert!(memory.get_all().is_empty());
        assert!(memory.get_recent(10).is_empty());
        assert_eq!(memory.token_count(), 0);
    }

    #[tokio::test]
    async fn test_in_memory_memory_default() {
        let memory: InMemoryMemory = Default::default();

        assert!(memory.is_empty());
    }

    #[tokio::test]
    async fn test_in_memory_memory_message_ordering() {
        let mut memory = InMemoryMemory::new();

        memory.add(Message::user("first")).await.unwrap();
        memory.add(Message::assistant("second")).await.unwrap();
        memory.add(Message::user("third")).await.unwrap();

        let all = memory.get_all();
        assert_eq!(all[0].content().as_text().unwrap(), "first");
        assert_eq!(all[1].content().as_text().unwrap(), "second");
        assert_eq!(all[2].content().as_text().unwrap(), "third");
    }

    #[tokio::test]
    async fn test_in_memory_memory_clone() {
        let mut memory1 = InMemoryMemory::new();

        memory1.add(Message::user("test")).await.unwrap();

        let mut memory2 = Clone::clone(&memory1);

        // Both should have the same underlying data
        assert_eq!(memory1.len(), memory2.len());
        assert_eq!(memory1.get_all().len(), memory2.get_all().len());

        // Changes to one should affect the other (same Arc)
        memory2.add(Message::assistant("response")).await.unwrap();

        assert_eq!(memory1.len(), 2);
        assert_eq!(memory2.len(), 2);
    }
}
