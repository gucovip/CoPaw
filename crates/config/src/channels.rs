use serde::{Deserialize, Serialize};

/// Base configuration for all channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseChannelConfig {
    pub enabled: bool,
    pub bot_prefix: String,
}

impl Default for BaseChannelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bot_prefix: String::new(),
        }
    }
}

/// iMessage channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IMessageChannelConfig {
    #[serde(flatten)]
    pub base: BaseChannelConfig,
    pub db_path: String,
    pub poll_sec: f64,
}

impl Default for IMessageChannelConfig {
    fn default() -> Self {
        Self {
            base: BaseChannelConfig::default(),
            db_path: "~/Library/Messages/chat.db".to_string(),
            poll_sec: 1.0,
        }
    }
}

/// Discord channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    #[serde(flatten)]
    pub base: BaseChannelConfig,
    pub bot_token: String,
    pub http_proxy: String,
    pub http_proxy_auth: String,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            base: BaseChannelConfig::default(),
            bot_token: String::new(),
            http_proxy: String::new(),
            http_proxy_auth: String::new(),
        }
    }
}

/// DingTalk channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkConfig {
    #[serde(flatten)]
    pub base: BaseChannelConfig,
    pub client_id: String,
    pub client_secret: String,
    pub media_dir: String,
}

impl Default for DingTalkConfig {
    fn default() -> Self {
        Self {
            base: BaseChannelConfig::default(),
            client_id: String::new(),
            client_secret: String::new(),
            media_dir: "~/.copaw/media".to_string(),
        }
    }
}

/// Feishu/Lark channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuConfig {
    #[serde(flatten)]
    pub base: BaseChannelConfig,
    pub app_id: String,
    pub app_secret: String,
    pub encrypt_key: String,
    pub verification_token: String,
    pub media_dir: String,
}

impl Default for FeishuConfig {
    fn default() -> Self {
        Self {
            base: BaseChannelConfig::default(),
            app_id: String::new(),
            app_secret: String::new(),
            encrypt_key: String::new(),
            verification_token: String::new(),
            media_dir: "~/.copaw/media".to_string(),
        }
    }
}

/// QQ channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QQConfig {
    #[serde(flatten)]
    pub base: BaseChannelConfig,
    pub app_id: String,
    pub client_secret: String,
}

impl Default for QQConfig {
    fn default() -> Self {
        Self {
            base: BaseChannelConfig::default(),
            app_id: String::new(),
            client_secret: String::new(),
        }
    }
}

/// Telegram channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    #[serde(flatten)]
    pub base: BaseChannelConfig,
    pub bot_token: String,
    pub http_proxy: String,
    pub http_proxy_auth: String,
    pub show_typing: Option<bool>,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            base: BaseChannelConfig::default(),
            bot_token: String::new(),
            http_proxy: String::new(),
            http_proxy_auth: String::new(),
            show_typing: None,
        }
    }
}

/// Console channel configuration (prints to stdout)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleConfig {
    pub enabled: bool,
}

impl Default for ConsoleConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Channel configuration container
/// Allows extra fields for plugin channels
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelConfig {
    pub imessage: IMessageChannelConfig,
    pub discord: DiscordConfig,
    pub dingtalk: DingTalkConfig,
    pub feishu: FeishuConfig,
    pub qq: QQConfig,
    pub telegram: TelegramConfig,
    pub console: ConsoleConfig,

    #[serde(flatten)]
    pub extra: serde_json::Value,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            imessage: IMessageChannelConfig::default(),
            discord: DiscordConfig::default(),
            dingtalk: DingTalkConfig::default(),
            feishu: FeishuConfig::default(),
            qq: QQConfig::default(),
            telegram: TelegramConfig::default(),
            console: ConsoleConfig::default(),
            extra: serde_json::Value::Object(Default::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_channel_config() {
        let config = ChannelConfig::default();
        assert_eq!(config.feishu.base.enabled, false);
        assert_eq!(config.console.enabled, true);
    }

    #[test]
    fn test_feishu_config_serialization() {
        let config = FeishuConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: FeishuConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.app_id, config.app_id);
        assert_eq!(parsed.media_dir, config.media_dir);
    }
}
