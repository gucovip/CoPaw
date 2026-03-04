// -*- coding: utf-8 -*-
// -*- mode: rust -*-
//! CoPaw Channels Crate
//!
//! This crate provides channel implementations for communicating through various
//! messaging platforms (Feishu, DingTalk, Discord, etc.) in the CoPaw system.
//!
//! Based on the Python implementation:
//! - src/copaw/app/channels/base.py - BaseChannel class
//! - src/copaw/app/channels/feishu/channel.py - Feishu channel
//! - src/copaw/app/channels/dingtalk/channel.py - DingTalk channel

pub mod feishu;
pub mod manager;

// Re-exports for convenience
pub use feishu::FeishuChannel;
pub use manager::{ChannelManager, ChannelManagerError, ChannelManagerResult};

#[cfg(test)]
mod tests {
    use super::*;
    use copaw_core::channel::{Channel, ChannelType};

    #[test]
    fn test_feishu_channel_creation() {
        let channel = FeishuChannel::new("test_app_id", "test_app_secret");
        assert_eq!(channel.channel_type(), ChannelType::Feishu);
    }

    #[test]
    fn test_feishu_channel_credentials() {
        let channel = FeishuChannel::new("my_app_id", "my_secret");
        assert_eq!(channel.app_id(), "my_app_id");
        assert_eq!(channel.app_secret(), "my_secret");
    }

    #[test]
    fn test_feishu_channel_default_state() {
        let channel = FeishuChannel::new("app_id", "secret");
        assert!(!channel.is_started());
    }

    #[tokio::test]
    async fn test_feishu_channel_start_stop() {
        let mut channel = FeishuChannel::new("app_id", "secret");

        // Test start
        let start_result = channel.start().await;
        assert!(start_result.is_ok());
        assert!(channel.is_started());

        // Test stop
        let stop_result = channel.stop().await;
        assert!(stop_result.is_ok());
        assert!(!channel.is_started());
    }

    #[tokio::test]
    async fn test_feishu_channel_send_text() {
        let mut channel = FeishuChannel::new("app_id", "secret");

        // Start the channel first
        channel.start().await.unwrap();

        let result = channel.send_text("test_user", "Hello, world!", None).await;
        assert!(result.is_ok());

        channel.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_feishu_channel_send_content_parts() {
        use copaw_core::message::ContentPart;

        let mut channel = FeishuChannel::new("app_id", "secret");
        channel.start().await.unwrap();

        let parts = vec![
            ContentPart::text("Hello"),
            ContentPart::thinking("Thinking..."),
        ];

        let result = channel.send_content_parts("test_user", parts, None).await;
        assert!(result.is_ok());

        channel.stop().await.unwrap();
    }

    #[test]
    fn test_feishu_resolve_session_id_default() {
        let channel = FeishuChannel::new("app_id", "secret");
        let session_id = channel.resolve_session_id("user123", None);
        assert_eq!(session_id, "feishu:user123");
    }

    #[test]
    fn test_feishu_resolve_session_id_with_conversation() {
        use serde_json::json;
        use std::collections::HashMap;

        let channel = FeishuChannel::new("app_id", "secret");

        let mut meta = HashMap::new();
        meta.insert(
            "conversation_id".to_string(),
            json!("oc_a0553eda9014c201e6969b4ad5de368c"),
        );

        // Feishu uses last 12 characters of conversation_id for session
        let session_id = channel.resolve_session_id("user123", Some(&meta));
        assert_eq!(session_id, "feishu:9b4ad5de368c");
    }

    #[tokio::test]
    async fn test_feishu_channel_send_with_metadata() {
        use serde_json::json;
        use std::collections::HashMap;

        let mut channel = FeishuChannel::new("app_id", "secret");
        channel.start().await.unwrap();

        let mut meta = HashMap::new();
        meta.insert("bot_prefix".to_string(), json!("[Bot] "));

        let result = channel
            .send_text("test_user", "Test message", Some(&meta))
            .await;
        assert!(result.is_ok());

        channel.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_feishu_channel_double_start_fails() {
        let mut channel = FeishuChannel::new("app_id", "secret");
        channel.start().await.unwrap();

        let result = channel.start().await;
        assert!(result.is_err());

        channel.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_feishu_channel_stop_without_start() {
        let mut channel = FeishuChannel::new("app_id", "secret");
        // Stopping without starting should be OK (no-op)
        let result = channel.stop().await;
        assert!(result.is_ok());
    }
}
