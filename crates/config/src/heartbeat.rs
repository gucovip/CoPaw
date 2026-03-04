use serde::{Deserialize, Serialize};

/// Optional active window for heartbeat (e.g. 08:00-22:00)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ActiveHoursConfig {
    pub start: String,
    pub end: String,
}

impl Default for ActiveHoursConfig {
    fn default() -> Self {
        Self {
            start: "08:00".to_string(),
            end: "22:00".to_string(),
        }
    }
}

/// Heartbeat configuration
/// Run agent with HEARTBEAT.md as query at interval
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HeartbeatConfig {
    /// Whether heartbeat is on
    pub enabled: bool,
    /// Interval (cron expression)
    pub every: String,
    /// Target channel/user for heartbeat
    pub target: String,
    /// Optional active hours window
    #[serde(rename = "activeHours", alias = "active_hours")]
    pub active_hours: Option<ActiveHoursConfig>,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            every: "0 9 * * *".to_string(), // Daily at 9 AM
            target: "".to_string(),
            active_hours: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_heartbeat_config() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.enabled, false);
        assert_eq!(config.every, "0 9 * * *");
    }

    #[test]
    fn test_active_hours_serialization() {
        let hours = ActiveHoursConfig::default();
        let json = serde_json::to_string(&hours).unwrap();
        assert!(json.contains("08:00"));
        assert!(json.contains("22:00"));
    }

    #[test]
    fn test_heartbeat_with_active_hours() {
        let config = HeartbeatConfig {
            enabled: true,
            every: "0 */1 * * *".to_string(),
            target: "console".to_string(),
            active_hours: Some(ActiveHoursConfig::default()),
        };
        assert!(config.active_hours.is_some());
    }
}
