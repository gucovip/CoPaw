use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use copaw_config::{
    get_config_path, load_config, save_config, ChannelConfig, CoPawConfig, ConsoleConfig,
    DingTalkConfig, DiscordConfig, FeishuConfig, HeartbeatConfig, QQConfig, TelegramConfig,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// State for configuration routes
#[derive(Clone)]
pub struct ConfigState {
    config_path: Option<PathBuf>,
}

impl ConfigState {
    pub fn new(config_path: Option<PathBuf>) -> Self {
        Self { config_path }
    }

    pub fn get_config_path(&self) -> PathBuf {
        self.config_path.clone().unwrap_or_else(get_config_path)
    }
}

/// Trait for accessing config path from state
pub trait HasConfigState {
    fn config_state(&self) -> &ConfigState;
}

impl HasConfigState for ConfigState {
    fn config_state(&self) -> &ConfigState {
        self
    }
}

/// Helper to get config path from any state implementing HasConfigState
fn get_config_path_from_state<S>(state: &S) -> PathBuf
where
    S: HasConfigState,
{
    state.config_state().get_config_path()
}

/// Error response structure
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// Custom error for config routes
#[derive(Debug)]
pub enum ConfigRouteError {
    LoadFailed(String),
    SaveFailed(String),
    ParseFailed(String),
    NotFound(String),
}

impl IntoResponse for ConfigRouteError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ConfigRouteError::LoadFailed(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ConfigRouteError::SaveFailed(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ConfigRouteError::ParseFailed(msg) => (StatusCode::BAD_REQUEST, msg),
            ConfigRouteError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };
        let body = ErrorResponse { error: message };
        (status, Json(body)).into_response()
    }
}

impl std::fmt::Display for ConfigRouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigRouteError::LoadFailed(msg) => write!(f, "Load failed: {}", msg),
            ConfigRouteError::SaveFailed(msg) => write!(f, "Save failed: {}", msg),
            ConfigRouteError::ParseFailed(msg) => write!(f, "Parse failed: {}", msg),
            ConfigRouteError::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
}

impl std::error::Error for ConfigRouteError {}

/// Channel configuration list response
#[derive(Serialize)]
struct ChannelListResponse {
    feishu: bool,
    discord: bool,
    dingtalk: bool,
    telegram: bool,
    qq: bool,
    imessage: bool,
    console: bool,
    #[serde(flatten)]
    extra: Value,
}

/// Channel configuration type
#[derive(Serialize)]
#[serde(tag = "type", content = "config")]
enum ChannelConfigResponse {
    Feishu(FeishuConfig),
    Discord(DiscordConfig),
    DingTalk(DingTalkConfig),
    Telegram(TelegramConfig),
    QQ(QQConfig),
    IMessage(Value), // IMessageChannelConfig doesn't have enabled flag
    Console(ConsoleConfig),
}

/// List available channel configurations
async fn list_channels<S>(
    State(state): State<S>,
) -> Result<Json<ChannelListResponse>, ConfigRouteError>
where
    S: HasConfigState,
{
    let config = load_config(Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::LoadFailed(e.to_string()))?;

    let channels = ChannelListResponse {
        feishu: config.channels.feishu.base.enabled,
        discord: config.channels.discord.base.enabled,
        dingtalk: config.channels.dingtalk.base.enabled,
        telegram: config.channels.telegram.base.enabled,
        qq: config.channels.qq.base.enabled,
        imessage: config.channels.imessage.base.enabled,
        console: config.channels.console.enabled,
        extra: config.channels.extra,
    };

    Ok(Json(channels))
}

/// List available channel types
async fn list_channel_types() -> Json<Value> {
    let types = serde_json::json!({
        "channels": [
            {"type": "feishu", "name": "Feishu/Lark"},
            {"type": "discord", "name": "Discord"},
            {"type": "dingtalk", "name": "DingTalk"},
            {"type": "telegram", "name": "Telegram"},
            {"type": "qq", "name": "QQ"},
            {"type": "imessage", "name": "iMessage"},
            {"type": "console", "name": "Console"}
        ]
    });
    Json(types)
}

/// Update channels configuration
#[derive(Deserialize)]
struct UpdateChannelsRequest {
    channels: ChannelConfig,
}

async fn put_channels<S>(
    State(state): State<S>,
    Json(req): Json<UpdateChannelsRequest>,
) -> Result<Json<Value>, ConfigRouteError>
where
    S: HasConfigState,
{
    let mut config = load_config(Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::LoadFailed(e.to_string()))?;

    config.channels = req.channels;

    save_config(&config, Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::SaveFailed(e.to_string()))?;

    Ok(Json(serde_json::json!({"success": true})))
}

/// Get specific channel configuration
async fn get_channel<S>(
    State(state): State<S>,
    Path(channel_name): Path<String>,
) -> Result<Json<ChannelConfigResponse>, ConfigRouteError>
where
    S: HasConfigState,
{
    let config = load_config(Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::LoadFailed(e.to_string()))?;

    let response = match channel_name.as_str() {
        "feishu" => ChannelConfigResponse::Feishu(config.channels.feishu),
        "discord" => ChannelConfigResponse::Discord(config.channels.discord),
        "dingtalk" => ChannelConfigResponse::DingTalk(config.channels.dingtalk),
        "telegram" => ChannelConfigResponse::Telegram(config.channels.telegram),
        "qq" => ChannelConfigResponse::QQ(config.channels.qq),
        "imessage" => ChannelConfigResponse::IMessage(
            serde_json::to_value(&config.channels.imessage)
                .map_err(|e| ConfigRouteError::ParseFailed(e.to_string()))?,
        ),
        "console" => ChannelConfigResponse::Console(config.channels.console),
        _ => {
            return Err(ConfigRouteError::NotFound(format!(
                "Unknown channel type: {}",
                channel_name
            )))
        }
    };

    Ok(Json(response))
}

/// Update specific channel configuration
#[derive(Deserialize)]
struct UpdateChannelRequest {
    #[serde(flatten)]
    config: Value,
}

async fn put_channel<S>(
    State(state): State<S>,
    Path(channel_name): Path<String>,
    Json(req): Json<UpdateChannelRequest>,
) -> Result<Json<Value>, ConfigRouteError>
where
    S: HasConfigState,
{
    let mut config = load_config(Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::LoadFailed(e.to_string()))?;

    match channel_name.as_str() {
        "feishu" => {
            config.channels.feishu = serde_json::from_value(req.config)
                .map_err(|e| ConfigRouteError::ParseFailed(e.to_string()))?;
        }
        "discord" => {
            config.channels.discord = serde_json::from_value(req.config)
                .map_err(|e| ConfigRouteError::ParseFailed(e.to_string()))?;
        }
        "dingtalk" => {
            config.channels.dingtalk = serde_json::from_value(req.config)
                .map_err(|e| ConfigRouteError::ParseFailed(e.to_string()))?;
        }
        "telegram" => {
            config.channels.telegram = serde_json::from_value(req.config)
                .map_err(|e| ConfigRouteError::ParseFailed(e.to_string()))?;
        }
        "qq" => {
            config.channels.qq = serde_json::from_value(req.config)
                .map_err(|e| ConfigRouteError::ParseFailed(e.to_string()))?;
        }
        "imessage" => {
            config.channels.imessage = serde_json::from_value(req.config)
                .map_err(|e| ConfigRouteError::ParseFailed(e.to_string()))?;
        }
        "console" => {
            config.channels.console = serde_json::from_value(req.config)
                .map_err(|e| ConfigRouteError::ParseFailed(e.to_string()))?;
        }
        _ => {
            return Err(ConfigRouteError::NotFound(format!(
                "Unknown channel type: {}",
                channel_name
            )))
        }
    }

    save_config(&config, Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::SaveFailed(e.to_string()))?;

    Ok(Json(serde_json::json!({"success": true})))
}

/// Get heartbeat configuration
async fn get_heartbeat<S>(State(state): State<S>) -> Result<Json<Value>, ConfigRouteError>
where
    S: HasConfigState,
{
    let config = load_config(Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::LoadFailed(e.to_string()))?;

    let heartbeat = config
        .agents
        .defaults
        .heartbeat
        .unwrap_or_else(HeartbeatConfig::default);

    Ok(Json(serde_json::to_value(&heartbeat).map_err(|e| {
        ConfigRouteError::ParseFailed(e.to_string())
    })?))
}

/// Update heartbeat configuration
#[derive(Deserialize)]
struct UpdateHeartbeatRequest {
    #[serde(flatten)]
    heartbeat: Value,
}

async fn put_heartbeat<S>(
    State(state): State<S>,
    Json(req): Json<UpdateHeartbeatRequest>,
) -> Result<Json<Value>, ConfigRouteError>
where
    S: HasConfigState,
{
    let mut config = load_config(Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::LoadFailed(e.to_string()))?;

    config.agents.defaults.heartbeat = serde_json::from_value(req.heartbeat)
        .map_err(|e| ConfigRouteError::ParseFailed(e.to_string()))?;

    save_config(&config, Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::SaveFailed(e.to_string()))?;

    Ok(Json(serde_json::json!({"success": true})))
}

/// Get full configuration
async fn get_config<S>(State(state): State<S>) -> Result<Json<CoPawConfig>, ConfigRouteError>
where
    S: HasConfigState,
{
    let config = load_config(Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::LoadFailed(e.to_string()))?;

    Ok(Json(config))
}

/// Update full configuration
async fn put_config<S>(
    State(state): State<S>,
    Json(new_config): Json<CoPawConfig>,
) -> Result<Json<Value>, ConfigRouteError>
where
    S: HasConfigState,
{
    save_config(&new_config, Some(&get_config_path_from_state(&state)))
        .await
        .map_err(|e| ConfigRouteError::SaveFailed(e.to_string()))?;

    Ok(Json(serde_json::json!({"success": true})))
}

/// Create the configuration router (generic over any state implementing HasConfigState)
pub fn create_config_router<S>() -> Router<S>
where
    S: HasConfigState + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/config", get(get_config::<S>).put(put_config::<S>))
        .route(
            "/config/channels",
            get(list_channels::<S>).put(put_channels::<S>),
        )
        .route("/config/channels/types", get(list_channel_types))
        .route(
            "/config/channels/:channel_name",
            get(get_channel::<S>).put(put_channel::<S>),
        )
        .route(
            "/config/heartbeat",
            get(get_heartbeat::<S>).put(put_heartbeat::<S>),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    async fn create_test_config() -> PathBuf {
        let config = CoPawConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        // Keep temp file around by forgetting it
        let _ = temp_file.into_temp_path();

        save_config(&config, Some(&path)).await.unwrap();
        path
    }

    #[tokio::test]
    async fn test_config_state() {
        let state = ConfigState::new(None);
        assert_eq!(state.get_config_path(), get_config_path());
    }

    #[tokio::test]
    async fn test_error_response() {
        let error = ConfigRouteError::NotFound("test".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_list_channels() {
        let config_path = create_test_config().await;
        let state = ConfigState::new(Some(config_path));

        let result = list_channels(State(state.clone())).await;
        assert!(result.is_ok());

        let channels = result.unwrap().0;
        assert!(channels.console); // Console is enabled by default
    }

    #[tokio::test]
    async fn test_list_channel_types() {
        let json = list_channel_types().await;
        assert!(json["channels"].is_array());
    }

    #[tokio::test]
    async fn test_put_channels() {
        let config_path = create_test_config().await;
        let state = ConfigState::new(Some(config_path));

        let req = UpdateChannelsRequest {
            channels: ChannelConfig::default(),
        };

        let result = put_channels(State(state), Json(req)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_channel_feishu() {
        let config_path = create_test_config().await;
        let state = ConfigState::new(Some(config_path));

        let result = get_channel(State(state), Path("feishu".to_string())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_channel_unknown() {
        let config_path = create_test_config().await;
        let state = ConfigState::new(Some(config_path));

        let result = get_channel(State(state), Path("unknown".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_heartbeat() {
        let config_path = create_test_config().await;
        let state = ConfigState::new(Some(config_path));

        let result = get_heartbeat(State(state)).await;
        assert!(result.is_ok());
        let heartbeat = result.unwrap().0;
        assert!(heartbeat.is_object());
        assert!(heartbeat.get("every").is_some());
    }

    #[tokio::test]
    async fn test_get_config() {
        let config_path = create_test_config().await;
        let state = ConfigState::new(Some(config_path));

        let result = get_config(State(state)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_put_config() {
        let config_path = create_test_config().await;
        let state = ConfigState::new(Some(config_path));

        let new_config = CoPawConfig::default();

        let result = put_config(State(state), Json(new_config)).await;
        assert!(result.is_ok());
    }
}
