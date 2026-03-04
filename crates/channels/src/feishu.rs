// -*- coding: utf-8 -*-
// -*- mode: rust -*-
//! Feishu (Lark) Channel Implementation
//!
//! This module provides the Feishu channel implementation for CoPaw.
//! Feishu is a collaboration platform by ByteDance (also known as Lark internationally).
//!
//! Based on the Python implementation:
//! - src/copaw/app/channels/feishu/channel.py
//!
//! The channel supports:
//! - WebSocket long connection for receiving events (no public IP required)
//! - Open API for sending messages (tenant_access_token)
//! - Text, image, and file messages
//! - Group chat context with chat_id and message_id metadata

use async_trait::async_trait;
use copaw_core::channel::{Channel, ChannelError, ChannelResult, ChannelType, Metadata};
use copaw_core::message::ContentPart;

/// Feishu Channel configuration and state
///
/// This struct holds the configuration and runtime state for the Feishu channel.
/// It implements the Channel trait to provide Feishu-specific messaging capabilities.
///
/// # Fields
///
/// * `app_id` - Feishu app ID for authentication
/// * `app_secret` - Feishu app secret for authentication
/// * `started` - Whether the channel is currently started
/// * `tenant_access_token` - Cached tenant access token for API calls
/// * `receive_id` - Saved receive_id for sending replies (from incoming messages)
#[derive(Debug, Clone)]
pub struct FeishuChannel {
    app_id: String,
    app_secret: String,
    started: bool,
    // Note: In a real implementation, these would be managed with proper synchronization
    // For this stub implementation, we're keeping it simple
    #[allow(dead_code)]
    tenant_access_token: Option<String>,
    #[allow(dead_code)]
    receive_id: Option<String>,
}

impl FeishuChannel {
    /// Create a new Feishu channel with the given credentials
    ///
    /// # Arguments
    ///
    /// * `app_id` - Feishu app ID
    /// * `app_secret` - Feishu app secret
    ///
    /// # Example
    ///
    /// ```rust
    /// use copaw_channels::FeishuChannel;
    ///
    /// let channel = FeishuChannel::new("cli_xxx", "my_secret");
    /// ```
    pub fn new<S: Into<String>>(app_id: S, app_secret: S) -> Self {
        FeishuChannel {
            app_id: app_id.into(),
            app_secret: app_secret.into(),
            started: false,
            tenant_access_token: None,
            receive_id: None,
        }
    }

    /// Get the app ID
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// Get the app secret
    ///
    /// Note: In production, you should be careful about exposing secrets in logs
    pub fn app_secret(&self) -> &str {
        &self.app_secret
    }

    /// Check if the channel is started
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Send a message via Feishu API
    ///
    /// This is a stub implementation. In the full implementation, this would:
    /// 1. Refresh tenant_access_token if needed
    /// 2. Call Feishu's message API (https://open.feishu.cn/open-apis/im/v1/messages)
    /// 3. Handle rate limits and errors appropriately
    ///
    /// # Arguments
    ///
    /// * `receive_id` - The target user or group ID to send to
    /// * `receive_id_type` - Type of receive_id (user_id, open_id, chat_id, etc.)
    /// * `content` - Message content (JSON string for different message types)
    /// * `msg_type` - Message type (text, post, interactive, etc.)
    async fn send_message(
        &mut self,
        _receive_id: &str,
        _receive_id_type: &str,
        _content: &str,
        _msg_type: &str,
    ) -> ChannelResult<()> {
        // Stub implementation - always succeeds for now
        // In real implementation, this would make an HTTP request to Feishu API
        Ok(())
    }

    /// Refresh tenant access token
    ///
    /// This is a stub implementation. In the full implementation, this would:
    /// 1. Call Feishu's auth API to get tenant_access_token
    /// 2. Cache the token with its expiration time
    /// 3. Return the cached token if not expired
    ///
    /// The token expires in 2 hours, so we need to refresh periodically.
    #[allow(dead_code)]
    async fn refresh_tenant_access_token(&mut self) -> ChannelResult<String> {
        // Stub implementation - return a fake token
        // In real implementation, this would call:
        // POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal
        Ok("fake_tenant_access_token".to_string())
    }
}

#[async_trait]
impl Channel for FeishuChannel {
    /// Get the channel type
    fn channel_type(&self) -> ChannelType {
        ChannelType::Feishu
    }

    /// Start the Feishu channel
    ///
    /// This is a stub implementation. In the full implementation, this would:
    /// 1. Initialize the lark-oapi client
    /// 2. Set up WebSocket long connection for receiving events
    /// 3. Register event handlers for message events
    ///
    /// The Python implementation uses lark-oapi's event handler system
    /// with a custom FeishuChannelHandler class.
    async fn start(&mut self) -> ChannelResult<()> {
        if self.started {
            return Err(ChannelError::StartError(
                "Channel already started".to_string(),
            ));
        }

        // Stub: Mark as started
        // In real implementation, this would initialize the WebSocket connection
        self.started = true;
        Ok(())
    }

    /// Stop the Feishu channel
    ///
    /// This is a stub implementation. In the full implementation, this would:
    /// 1. Close the WebSocket connection
    /// 2. Clean up event handlers
    /// 3. Release resources
    async fn stop(&mut self) -> ChannelResult<()> {
        if !self.started {
            // Stopping when not started is OK (no-op)
            return Ok(());
        }

        // Stub: Mark as stopped
        // In real implementation, this would close connections
        self.started = false;
        Ok(())
    }

    /// Send a text message to a Feishu user or group
    ///
    /// This is a stub implementation. In the full implementation, this would:
    /// 1. Get receive_id from to_handle or saved state
    /// 2. Format the text message according to Feishu's API format
    /// 3. Call send_message with msg_type="text"
    ///
    /// # Arguments
    ///
    /// * `to_handle` - The target handle (user_id, open_id, chat_id)
    /// * `text` - The text message to send
    /// * `meta` - Optional metadata (bot_prefix, etc.)
    async fn send_text(
        &mut self,
        to_handle: &str,
        text: &str,
        meta: Option<&Metadata>,
    ) -> ChannelResult<()> {
        if !self.started {
            return Err(ChannelError::NotConnected);
        }

        // Apply bot_prefix if present in metadata
        let bot_prefix = meta
            .and_then(|m| m.get("bot_prefix"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let message = format!("{}{}", bot_prefix, text);

        // Stub implementation
        self.send_message(to_handle, "open_id", &message, "text")
            .await?;

        Ok(())
    }

    /// Send structured content parts to a Feishu user or group
    ///
    /// This is a stub implementation. In the full implementation, this would:
    /// 1. Process each content part (text, image, file, etc.)
    /// 2. For text parts, send as text message
    /// 3. For image parts, upload image and send with image_key
    /// 4. For file parts, upload file and send with file_key
    /// 5. Merge multiple parts appropriately
    ///
    /// # Arguments
    ///
    /// * `to_handle` - The target handle (user_id, open_id, chat_id)
    /// * `parts` - The content parts to send
    /// * `meta` - Optional metadata
    async fn send_content_parts(
        &mut self,
        to_handle: &str,
        parts: Vec<ContentPart>,
        meta: Option<&Metadata>,
    ) -> ChannelResult<()> {
        if !self.started {
            return Err(ChannelError::NotConnected);
        }

        // Collect text parts into a single message
        let mut text_parts = Vec::new();

        for part in &parts {
            match part {
                ContentPart::Text { text } => {
                    text_parts.push(text.as_str());
                }
                ContentPart::Thinking { thinking: _ } => {
                    // Skip thinking parts in Feishu (they're internal)
                    // Or could include as italic text
                }
                _ => {
                    // For now, just note that other content types exist
                    // In full implementation, would handle images, files, etc.
                }
            }
        }

        if !text_parts.is_empty() {
            let combined_text = text_parts.join("\n");
            self.send_text(to_handle, &combined_text, meta).await?;
        }

        Ok(())
    }

    /// Resolve a session ID from sender ID and optional metadata
    ///
    /// Feishu-specific implementation:
    /// - If conversation_id is present in metadata, use last 12 chars
    /// - This enables cron job lookup by conversation_id suffix
    /// - Otherwise defaults to "feishu:{sender_id}"
    ///
    /// # Arguments
    ///
    /// * `sender_id` - The sender's identifier
    /// * `meta` - Optional channel metadata (may contain conversation_id)
    fn resolve_session_id(&self, sender_id: &str, meta: Option<&Metadata>) -> String {
        // Check for conversation_id in metadata (Feishu-specific)
        if let Some(m) = meta {
            if let Some(conv_id_value) = m.get("conversation_id") {
                if let Some(conv_id) = conv_id_value.as_str() {
                    // Use last 12 characters for shorter IDs (same as Python)
                    let short_id = if conv_id.len() > 12 {
                        &conv_id[conv_id.len() - 12..]
                    } else {
                        conv_id
                    };
                    return format!("feishu:{}", short_id);
                }
            }
        }

        // Default implementation
        format!("feishu:{}", sender_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feishu_channel_new() {
        let channel = FeishuChannel::new("app_id", "secret");
        assert_eq!(channel.app_id(), "app_id");
        assert_eq!(channel.app_secret(), "secret");
        assert!(!channel.is_started());
    }

    #[test]
    fn test_feishu_channel_type() {
        let channel = FeishuChannel::new("app_id", "secret");
        assert_eq!(channel.channel_type(), ChannelType::Feishu);
    }

    #[tokio::test]
    async fn test_feishu_channel_start_stop() {
        let mut channel = FeishuChannel::new("app_id", "secret");

        // Start
        channel.start().await.unwrap();
        assert!(channel.is_started());

        // Stop
        channel.stop().await.unwrap();
        assert!(!channel.is_started());
    }

    #[tokio::test]
    async fn test_feishu_channel_double_start() {
        let mut channel = FeishuChannel::new("app_id", "secret");
        channel.start().await.unwrap();

        let result = channel.start().await;
        assert!(result.is_err());
        match result {
            Err(ChannelError::StartError(msg)) => {
                assert!(msg.contains("already started"));
            }
            _ => panic!("Expected StartError"),
        }
    }

    #[tokio::test]
    async fn test_feishu_channel_stop_without_start() {
        let mut channel = FeishuChannel::new("app_id", "secret");
        // Should not error
        channel.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_feishu_channel_send_text() {
        let mut channel = FeishuChannel::new("app_id", "secret");
        channel.start().await.unwrap();

        let result = channel.send_text("test_user", "Hello", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_feishu_channel_send_text_without_start() {
        let mut channel = FeishuChannel::new("app_id", "secret");

        let result = channel.send_text("test_user", "Hello", None).await;
        assert!(result.is_err());
        match result {
            Err(ChannelError::NotConnected) => {}
            _ => panic!("Expected NotConnected error"),
        }
    }

    #[tokio::test]
    async fn test_feishu_channel_send_content_parts() {
        let mut channel = FeishuChannel::new("app_id", "secret");
        channel.start().await.unwrap();

        let parts = vec![
            ContentPart::text("Hello"),
            ContentPart::thinking("Thinking..."),
            ContentPart::text("World"),
        ];

        let result = channel.send_content_parts("test_user", parts, None).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_feishu_resolve_session_id_default() {
        let channel = FeishuChannel::new("app_id", "secret");
        let session_id = channel.resolve_session_id("user123", None);
        assert_eq!(session_id, "feishu:user123");
    }

    #[test]
    fn test_feishu_resolve_session_id_with_conversation() {
        let channel = FeishuChannel::new("app_id", "secret");

        let mut meta = Metadata::new();
        meta.insert(
            "conversation_id".to_string(),
            serde_json::json!("oc_a0553eda9014c201e6969b4ad5de368c"),
        );

        let session_id = channel.resolve_session_id("user123", Some(&meta));
        assert_eq!(session_id, "feishu:9b4ad5de368c");
    }

    #[test]
    fn test_feishu_resolve_session_id_short_conversation() {
        let channel = FeishuChannel::new("app_id", "secret");

        let mut meta = Metadata::new();
        meta.insert("conversation_id".to_string(), serde_json::json!("short"));

        let session_id = channel.resolve_session_id("user123", Some(&meta));
        assert_eq!(session_id, "feishu:short");
    }
}
