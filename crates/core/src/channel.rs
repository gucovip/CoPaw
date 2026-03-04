// -*- coding: utf-8 -*-
// -*- mode: rust -*-
/**
 * Channel trait for CoPaw communication channels.
 *
 * This module defines the Channel trait and related types for implementing
 * different communication channels (DingTalk, Feishu, Discord, etc.) in CoPaw.
 *
 * Based on the Python implementation:
 * - src/copaw/app/channels/base.py - BaseChannel class
 * - src/copaw/app/channels/schema.py - ChannelType and related types
 */
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Channel type identifiers
///
/// These correspond to the built-in channel types in CoPaw.
/// Plugin channels can use custom string values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelType {
    /// Apple Messages channel
    IMessage,
    /// Discord channel
    Discord,
    /// DingTalk channel
    DingTalk,
    /// Feishu (Lark) channel
    Feishu,
    /// QQ channel
    QQ,
    /// Telegram channel
    Telegram,
    /// Console channel (default)
    Console,
    /// Custom plugin channel with string identifier
    #[serde(untagged)]
    Custom(String),
}

impl ChannelType {
    /// Get the string representation of the channel type
    pub fn as_str(&self) -> &str {
        match self {
            ChannelType::IMessage => "imessage",
            ChannelType::Discord => "discord",
            ChannelType::DingTalk => "dingtalk",
            ChannelType::Feishu => "feishu",
            ChannelType::QQ => "qq",
            ChannelType::Telegram => "telegram",
            ChannelType::Console => "console",
            ChannelType::Custom(s) => s,
        }
    }
}

impl FromStr for ChannelType {
    type Err = ChannelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "imessage" => ChannelType::IMessage,
            "discord" => ChannelType::Discord,
            "dingtalk" => ChannelType::DingTalk,
            "feishu" => ChannelType::Feishu,
            "qq" => ChannelType::QQ,
            "telegram" => ChannelType::Telegram,
            "console" => ChannelType::Console,
            other => ChannelType::Custom(other.to_string()),
        })
    }
}

impl fmt::Display for ChannelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<String> for ChannelType {
    fn from(s: String) -> Self {
        ChannelType::from_str(&s).expect("ChannelType::from_str should never fail")
    }
}

impl From<&str> for ChannelType {
    fn from(s: &str) -> Self {
        ChannelType::from_str(s).expect("ChannelType::from_str should never fail")
    }
}

impl AsRef<str> for ChannelType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Metadata type for channel operations
///
/// This is a HashMap that can contain arbitrary metadata
/// for channel operations (e.g., webhook URLs, conversation IDs, etc.)
pub type Metadata = HashMap<String, serde_json::Value>;

/// Errors that can occur during channel operations
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ChannelError {
    /// Generic channel error
    #[error("channel error: {0}")]
    ChannelError(String),

    /// Send operation failed
    #[error("send failed: {0}")]
    SendError(String),

    /// Start operation failed
    #[error("start failed: {0}")]
    StartError(String),

    /// Stop operation failed
    #[error("stop failed: {0}")]
    StopError(String),

    /// Invalid configuration
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Authentication failed
    #[error("authentication failed: {0}")]
    AuthError(String),

    /// Rate limit exceeded
    #[error("rate limit exceeded")]
    RateLimited,

    /// Network error
    #[error("network error: {0}")]
    NetworkError(String),

    /// Parsing error
    #[error("parse error: {0}")]
    ParseError(String),

    /// Not implemented
    #[error("not implemented: {0}")]
    NotImplemented(String),

    /// Channel not connected
    #[error("channel not connected")]
    NotConnected,

    /// Operation timed out
    #[error("operation timed out")]
    Timeout,
}

/// Result type for channel operations
pub type ChannelResult<T> = Result<T, ChannelError>;

/// Channel trait for implementing communication channels
///
/// This trait defines the interface that all channel implementations must follow.
/// Channels are responsible for:
/// - Starting and stopping the connection
/// - Sending messages to users
/// - Managing session IDs
///
/// Based on Python's BaseChannel class.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Get the channel type
    fn channel_type(&self) -> ChannelType;

    /// Start the channel
    ///
    /// This should establish any necessary connections and begin listening for messages.
    async fn start(&mut self) -> ChannelResult<()>;

    /// Stop the channel
    ///
    /// This should gracefully close connections and cleanup resources.
    async fn stop(&mut self) -> ChannelResult<()>;

    /// Send a text message to a user
    ///
    /// # Arguments
    /// * `to_handle` - The target handle (user ID, channel ID, etc.)
    /// * `text` - The text message to send
    /// * `meta` - Optional metadata for the send operation
    async fn send_text(
        &mut self,
        to_handle: &str,
        text: &str,
        meta: Option<&Metadata>,
    ) -> ChannelResult<()>;

    /// Send structured content parts to a user
    ///
    /// # Arguments
    /// * `to_handle` - The target handle (user ID, channel ID, etc.)
    /// * `parts` - The content parts to send (text, images, etc.)
    /// * `meta` - Optional metadata for the send operation
    async fn send_content_parts(
        &mut self,
        to_handle: &str,
        parts: Vec<crate::message::ContentPart>,
        meta: Option<&Metadata>,
    ) -> ChannelResult<()>;

    /// Resolve a session ID from sender ID and optional metadata
    ///
    /// The default implementation uses the format "{channel_type}:{sender_id}".
    /// Channels can override this for custom session ID logic (e.g., using
    /// conversation ID suffixes).
    ///
    /// # Arguments
    /// * `sender_id` - The sender's identifier
    /// * `meta` - Optional channel metadata
    fn resolve_session_id(&self, sender_id: &str, meta: Option<&Metadata>) -> String {
        match meta {
            Some(m) if m.contains_key("conversation_id") => {
                if let Some(conv_id) = m.get("conversation_id").and_then(|v| v.as_str()) {
                    // Use conversation_id if available (e.g., for cron lookup)
                    // Use last 12 chars for shorter IDs
                    let short_id = if conv_id.len() > 12 {
                        &conv_id[conv_id.len() - 12..]
                    } else {
                        conv_id
                    };
                    return format!("{}:{}", self.channel_type(), short_id);
                }
            }
            _ => {}
        }
        format!("{}:{}", self.channel_type(), sender_id)
    }

    /// Clone the channel with updated configuration
    ///
    /// The default implementation returns an error. Channels that support
    /// hot-reload should implement this to return a new instance with updated config.
    ///
    /// # Arguments
    /// * `config` - The new configuration (as a JSON value)
    fn clone_channel(&self, _config: &serde_json::Value) -> ChannelResult<Box<dyn Channel>> {
        Err(ChannelError::NotImplemented(
            "clone_channel not implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::ContentPart;

    struct MockChannel {
        channel_type: ChannelType,
        started: bool,
    }

    #[async_trait::async_trait]
    impl Channel for MockChannel {
        fn channel_type(&self) -> ChannelType {
            self.channel_type.clone()
        }

        async fn start(&mut self) -> ChannelResult<()> {
            self.started = true;
            Ok(())
        }

        async fn stop(&mut self) -> ChannelResult<()> {
            self.started = false;
            Ok(())
        }

        async fn send_text(
            &mut self,
            _to_handle: &str,
            _text: &str,
            _meta: Option<&Metadata>,
        ) -> ChannelResult<()> {
            Ok(())
        }

        async fn send_content_parts(
            &mut self,
            _to_handle: &str,
            _parts: Vec<ContentPart>,
            _meta: Option<&Metadata>,
        ) -> ChannelResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_channel_type_display() {
        assert_eq!(ChannelType::IMessage.as_str(), "imessage");
        assert_eq!(ChannelType::Discord.as_str(), "discord");
        assert_eq!(ChannelType::DingTalk.as_str(), "dingtalk");
        assert_eq!(ChannelType::Feishu.as_str(), "feishu");
        assert_eq!(ChannelType::QQ.as_str(), "qq");
        assert_eq!(ChannelType::Telegram.as_str(), "telegram");
        assert_eq!(ChannelType::Console.as_str(), "console");
    }

    #[test]
    fn test_channel_type_from_str() {
        assert_eq!(
            ChannelType::from_str("imessage").unwrap(),
            ChannelType::IMessage
        );
        assert_eq!(
            ChannelType::from_str("Discord").unwrap(),
            ChannelType::Discord
        );
        assert_eq!(
            ChannelType::from_str("DINGTALK").unwrap(),
            ChannelType::DingTalk
        );
        assert_eq!(
            ChannelType::from_str("feishu").unwrap(),
            ChannelType::Feishu
        );
        assert_eq!(ChannelType::from_str("QQ").unwrap(), ChannelType::QQ);
        assert_eq!(
            ChannelType::from_str("telegram").unwrap(),
            ChannelType::Telegram
        );
        assert_eq!(
            ChannelType::from_str("console").unwrap(),
            ChannelType::Console
        );

        // Custom channel
        match ChannelType::from_str("custom_channel").unwrap() {
            ChannelType::Custom(s) => assert_eq!(s, "custom_channel"),
            _ => panic!("Expected Custom variant"),
        }
    }

    #[test]
    fn test_channel_type_from_string() {
        let s: String = "feishu".to_string();
        assert_eq!(ChannelType::from(s.clone()), ChannelType::Feishu);
        assert_eq!(ChannelType::from(s.as_str()), ChannelType::Feishu);
    }

    #[test]
    fn test_channel_type_display_trait() {
        assert_eq!(ChannelType::Feishu.to_string(), "feishu");
        assert_eq!(format!("{}", ChannelType::Discord), "discord");
    }

    #[test]
    fn test_channel_type_as_ref() {
        assert_eq!(ChannelType::QQ.as_ref(), "qq");
        assert_eq!(
            <ChannelType as AsRef<str>>::as_ref(&ChannelType::Telegram),
            "telegram"
        );
    }

    #[test]
    fn test_channel_error_display() {
        let err = ChannelError::SendError("test error".to_string());
        assert_eq!(err.to_string(), "send failed: test error");

        let err = ChannelError::RateLimited;
        assert_eq!(err.to_string(), "rate limit exceeded");

        let err = ChannelError::NotConnected;
        assert_eq!(err.to_string(), "channel not connected");
    }

    #[tokio::test]
    async fn test_channel_type_method() {
        let channel = MockChannel {
            channel_type: ChannelType::Feishu,
            started: false,
        };
        assert_eq!(channel.channel_type(), ChannelType::Feishu);
    }

    #[tokio::test]
    async fn test_channel_start_stop() {
        let mut channel = MockChannel {
            channel_type: ChannelType::Discord,
            started: false,
        };

        channel.start().await.unwrap();
        assert!(channel.started);

        channel.stop().await.unwrap();
        assert!(!channel.started);
    }

    #[tokio::test]
    async fn test_channel_send_text() {
        let mut channel = MockChannel {
            channel_type: ChannelType::Telegram,
            started: false,
        };

        let mut meta = Metadata::new();
        meta.insert("key".to_string(), serde_json::json!("value"));

        let result = channel
            .send_text("@user", "Hello, world!", Some(&meta))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_channel_send_content_parts() {
        let mut channel = MockChannel {
            channel_type: ChannelType::QQ,
            started: false,
        };

        let parts = vec![
            ContentPart::text("Hello"),
            ContentPart::thinking("Thinking..."),
        ];

        let result = channel.send_content_parts("@user", parts, None).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_session_id_default() {
        let channel = MockChannel {
            channel_type: ChannelType::DingTalk,
            started: false,
        };

        // Default implementation
        let session_id = channel.resolve_session_id("user123", None);
        assert_eq!(session_id, "dingtalk:user123");
    }

    #[test]
    fn test_resolve_session_id_with_conversation_id() {
        let channel = MockChannel {
            channel_type: ChannelType::Feishu,
            started: false,
        };

        // With conversation_id in metadata
        let mut meta = Metadata::new();
        meta.insert(
            "conversation_id".to_string(),
            serde_json::json!("oc_a0553eda9014c201e6969b4ad5de368c"),
        );

        let session_id = channel.resolve_session_id("user123", Some(&meta));
        // Should use last 12 chars of conversation_id
        // "oc_a0553eda9014c201e6969b4ad5de368c" -> "9b4ad5de368c"
        assert_eq!(session_id, "feishu:9b4ad5de368c");
    }

    #[test]
    fn test_resolve_session_id_short_conversation_id() {
        let channel = MockChannel {
            channel_type: ChannelType::Discord,
            started: false,
        };

        // With short conversation_id in metadata
        let mut meta = Metadata::new();
        meta.insert("conversation_id".to_string(), serde_json::json!("short"));

        let session_id = channel.resolve_session_id("user123", Some(&meta));
        assert_eq!(session_id, "discord:short");
    }

    #[test]
    fn test_clone_channel_default() {
        let channel = MockChannel {
            channel_type: ChannelType::Console,
            started: false,
        };

        let result = channel.clone_channel(&serde_json::json!({}));
        assert!(result.is_err());
        match result {
            Err(ChannelError::NotImplemented(msg)) => {
                assert!(msg.contains("clone_channel"));
            }
            _ => panic!("Expected NotImplemented error"),
        }
    }

    #[test]
    fn test_metadata_type() {
        let mut meta = Metadata::new();
        meta.insert(
            "webhook".to_string(),
            serde_json::json!("https://example.com"),
        );
        meta.insert("user_id".to_string(), serde_json::json!(123));
        meta.insert("nested".to_string(), serde_json::json!({"key": "value"}));

        assert_eq!(
            meta.get("webhook"),
            Some(&serde_json::json!("https://example.com"))
        );
        assert_eq!(meta.get("user_id"), Some(&serde_json::json!(123)));
    }

    #[test]
    fn test_channel_error_equality() {
        let err1 = ChannelError::SendError("test".to_string());
        let err2 = ChannelError::SendError("test".to_string());
        assert_eq!(err1, err2);

        let err3 = ChannelError::SendError("other".to_string());
        assert_ne!(err1, err3);

        let err4 = ChannelError::RateLimited;
        let err5 = ChannelError::RateLimited;
        assert_eq!(err4, err5);
    }

    #[tokio::test]
    async fn test_multiple_channel_types() {
        let types = vec![
            ChannelType::IMessage,
            ChannelType::Discord,
            ChannelType::DingTalk,
            ChannelType::Feishu,
            ChannelType::QQ,
            ChannelType::Telegram,
            ChannelType::Console,
        ];

        for channel_type in types {
            let channel = MockChannel {
                channel_type: channel_type.clone(),
                started: false,
            };
            assert_eq!(channel.channel_type(), channel_type);
            assert_eq!(channel.channel_type().as_str(), channel_type.as_str());
        }
    }
}
