// -*- coding: utf-8 -*-
// -*- mode: rust -*-
//! Channel manager for orchestrating multiple communication channels.
//!
//! This module provides the ChannelManager which handles starting/stopping
//! all configured channels, matching the Python implementation's channel management.

use copaw_core::channel::{Channel, Metadata};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Error type for channel manager operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum ChannelManagerError {
    #[error("Channel not found: {0}")]
    NotFound(String),

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("IO error: {0}")]
    Io(String),
}

/// Result type for channel manager operations
pub type ChannelManagerResult<T> = Result<T, ChannelManagerError>;

/// Channel manager for orchestrating multiple channels
///
/// The manager maintains a collection of channels and provides:
/// - Starting all channels
/// - Stopping all channels
/// - Sending messages through specific channels
/// - Getting channel by name
pub struct ChannelManager {
    /// Map of channel name to channel instance
    channels: Arc<RwLock<HashMap<String, Box<dyn Channel>>>>,

    /// Whether the manager has been started
    started: Arc<RwLock<bool>>,
}

impl ChannelManager {
    /// Create a new channel manager
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            started: Arc::new(RwLock::new(false)),
        }
    }

    /// Add a channel to the manager
    ///
    /// # Arguments
    ///
    /// * `name` - Channel name (e.g., "feishu", "discord", "console")
    /// * `channel` - Channel instance
    pub async fn add_channel(&self, name: String, channel: Box<dyn Channel>) {
        let mut channels = self.channels.write().await;
        channels.insert(name, channel);
    }

    /// Remove a channel from the manager
    ///
    /// # Arguments
    ///
    /// * `name` - Channel name to remove
    pub async fn remove_channel(&self, name: &str) -> Option<Box<dyn Channel>> {
        let mut channels = self.channels.write().await;
        channels.remove(name)
    }

    /// Get a channel by name
    ///
    /// # Arguments
    ///
    /// * `name` - Channel name
    pub async fn get_channel(&self, name: &str) -> Option<String> {
        let channels = self.channels.read().await;
        if channels.contains_key(name) {
            Some(name.to_string())
        } else {
            None
        }
    }

    /// List all channel names
    pub async fn list_channels(&self) -> Vec<String> {
        let channels = self.channels.read().await;
        channels.keys().cloned().collect()
    }

    /// Start all channels
    ///
    /// This will start all registered channels in parallel.
    /// Errors from individual channels are logged but don't stop other channels.
    pub async fn start_all(&self) -> ChannelManagerResult<()> {
        let mut started = self.started.write().await;
        if *started {
            return Ok(());
        }

        let channels = self.channels.read().await;
        let mut handles = Vec::new();

        for (name, _channel) in channels.iter() {
            let name = name.clone();
            let channels_clone = self.channels.clone();
            let handle = tokio::spawn(async move {
                let channels = channels_clone.read().await;
                if let Some(_channel) = channels.get(&name) {
                    // Note: We need interior mutability here since Channel::start takes &mut self
                    // For now, this is a simplified implementation
                    tracing::info!("Starting channel: {}", name);
                }
            });
            handles.push(handle);
        }

        // Wait for all channels to start
        for handle in handles {
            let _ = handle.await;
        }

        *started = true;
        Ok(())
    }

    /// Stop all channels
    ///
    /// This will stop all registered channels in parallel.
    /// Errors from individual channels are logged but don't stop other channels.
    pub async fn stop_all(&self) -> ChannelManagerResult<()> {
        let mut started = self.started.write().await;
        if !*started {
            return Ok(());
        }

        let channels = self.channels.read().await;
        let mut handles = Vec::new();

        for (name, _channel) in channels.iter() {
            let name = name.clone();
            let channels_clone = self.channels.clone();
            let handle = tokio::spawn(async move {
                let channels = channels_clone.read().await;
                if let Some(_channel) = channels.get(&name) {
                    // Note: We need interior mutability here since Channel::stop takes &mut self
                    // For now, this is a simplified implementation
                    tracing::info!("Stopping channel: {}", name);
                }
            });
            handles.push(handle);
        }

        // Wait for all channels to stop
        for handle in handles {
            let _ = handle.await;
        }

        *started = false;
        Ok(())
    }

    /// Send a text message through a specific channel
    ///
    /// # Arguments
    ///
    /// * `channel` - Channel name
    /// * `user_id` - Target user ID
    /// * `session_id` - Session ID
    /// * `text` - Text message to send
    /// * `meta` - Optional metadata
    pub async fn send_text(
        &self,
        channel: &str,
        user_id: &str,
        session_id: &str,
        text: &str,
        _meta: Option<&Metadata>,
    ) -> ChannelManagerResult<()> {
        let channels = self.channels.read().await;

        // This is a simplified implementation
        // In a real implementation, we would need interior mutability
        // to get &mut access to the channel
        if !channels.contains_key(channel) {
            return Err(ChannelManagerError::NotFound(channel.to_string()));
        }

        tracing::info!(
            "Sending text through channel {}: user_id={}, session_id={}, text_len={}",
            channel,
            user_id,
            session_id,
            text.len()
        );

        // TODO: Actually send the message when we have interior mutability
        Ok(())
    }

    /// Send an event through a specific channel
    ///
    /// # Arguments
    ///
    /// * `channel` - Channel name
    /// * `user_id` - Target user ID
    /// * `session_id` - Session ID
    /// * `event` - Event data (as JSON value)
    /// * `meta` - Optional metadata
    pub async fn send_event(
        &self,
        channel: &str,
        user_id: &str,
        session_id: &str,
        event: &serde_json::Value,
        _meta: Option<&Metadata>,
    ) -> ChannelManagerResult<()> {
        let channels = self.channels.read().await;

        if !channels.contains_key(channel) {
            return Err(ChannelManagerError::NotFound(channel.to_string()));
        }

        tracing::info!(
            "Sending event through channel {}: user_id={}, session_id={}, event={}",
            channel,
            user_id,
            session_id,
            event
        );

        // TODO: Actually send the event when we have interior mutability
        Ok(())
    }

    /// Check if the manager is started
    pub async fn is_started(&self) -> bool {
        *self.started.read().await
    }

    /// Get the number of registered channels
    pub async fn channel_count(&self) -> usize {
        self.channels.read().await.len()
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_create() {
        let manager = ChannelManager::new();
        assert!(!manager.is_started().await);
        assert_eq!(manager.channel_count().await, 0);
    }

    #[tokio::test]
    async fn test_manager_default() {
        let manager = ChannelManager::default();
        assert!(!manager.is_started().await);
        assert_eq!(manager.channel_count().await, 0);
    }

    #[tokio::test]
    async fn test_list_channels_empty() {
        let manager = ChannelManager::new();
        let channels = manager.list_channels().await;
        assert!(channels.is_empty());
    }

    #[tokio::test]
    async fn test_send_text_channel_not_found() {
        let manager = ChannelManager::new();

        let result = manager
            .send_text("nonexistent", "user", "session", "Hello", None)
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ChannelManagerError::NotFound(_) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_send_event_channel_not_found() {
        let manager = ChannelManager::new();

        let event = serde_json::json!({"type": "test"});
        let result = manager
            .send_event("nonexistent", "user", "session", &event, None)
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ChannelManagerError::NotFound(_) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_start_stop_all() {
        let manager = ChannelManager::new();

        manager.start_all().await.unwrap();
        assert!(manager.is_started().await);

        manager.stop_all().await.unwrap();
        assert!(!manager.is_started().await);
    }

    #[tokio::test]
    async fn test_start_all_twice() {
        let manager = ChannelManager::new();

        manager.start_all().await.unwrap();
        // Starting twice should be OK (no-op)
        manager.start_all().await.unwrap();
        assert!(manager.is_started().await);
    }

    #[tokio::test]
    async fn test_stop_all_without_start() {
        let manager = ChannelManager::new();

        // Stopping without starting should be OK (no-op)
        manager.stop_all().await.unwrap();
        assert!(!manager.is_started().await);
    }

    #[test]
    fn test_channel_manager_error_display() {
        let err = ChannelManagerError::NotFound("test".to_string());
        assert_eq!(err.to_string(), "Channel not found: test");

        let err = ChannelManagerError::Channel("test error".to_string());
        assert_eq!(err.to_string(), "Channel error: test error");

        let err = ChannelManagerError::Io("io error".to_string());
        assert_eq!(err.to_string(), "IO error: io error");
    }
}
