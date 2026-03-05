// -*- coding: utf-8 -*-
// -*- mode: rust -*-
//! Cron job models for CoPaw.
//!
//! This module defines the data structures for cron jobs,
//! matching the Python implementation in src/copaw/app/crons/models.py.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Schedule specification for cron jobs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ScheduleSpec {
    /// Type of schedule (only "cron" is supported)
    #[serde(default = "default_schedule_type")]
    pub r#type: String,

    /// Cron expression (5 fields: minute hour day month day_of_week)
    pub cron: String,

    /// Timezone for the schedule (default: UTC)
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

fn default_schedule_type() -> String {
    "cron".to_string()
}

fn default_timezone() -> String {
    "UTC".to_string()
}

impl ScheduleSpec {
    /// Validate and normalize the cron expression
    /// Outputs 6-field format (seconds minute hour day month day_of_week) for tokio-cron-scheduler
    pub fn normalize_cron(cron: &str) -> Result<String, String> {
        let parts: Vec<&str> = cron.split_whitespace().collect();

        match parts.len() {
            5 => {
                // Add seconds field (default to 0)
                Ok(format!("0 {}", parts.join(" ")))
            }
            4 => {
                // treat as: hour dom month dow -> 0 minute hour dom month dow
                Ok(format!(
                    "0 0 {} {} {} {}",
                    parts[0], parts[1], parts[2], parts[3]
                ))
            }
            3 => {
                // treat as: dom month dow -> 0 0 minute dom month dow
                Ok(format!("0 0 0 {} {} {}", parts[0], parts[1], parts[2]))
            }
            _ => Err(format!(
                "cron must have 5 fields (or 4/3 fields that can be normalized); got {}",
                parts.len()
            )),
        }
    }
}

/// Dispatch target for cron jobs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DispatchTarget {
    /// User ID to send the result to
    pub user_id: String,

    /// Session ID for the conversation
    pub session_id: String,
}

/// Dispatch specification for cron jobs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct DispatchSpec {
    /// Type of dispatch (only "channel" is supported)
    #[serde(default = "default_dispatch_type")]
    pub r#type: String,

    /// Channel name to use for dispatch
    #[serde(default = "default_channel")]
    pub channel: String,

    /// Target for the dispatch
    pub target: DispatchTarget,

    /// Mode of dispatch ("stream" or "final")
    #[serde(default = "default_dispatch_mode")]
    pub mode: String,

    /// Additional metadata for dispatch
    #[serde(default)]
    pub meta: HashMap<String, Value>,
}

fn default_dispatch_type() -> String {
    "channel".to_string()
}

fn default_channel() -> String {
    "console".to_string()
}

fn default_dispatch_mode() -> String {
    "stream".to_string()
}

/// Runtime specification for cron jobs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobRuntimeSpec {
    /// Maximum concurrent executions
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,

    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: usize,

    /// Grace period for misfired jobs in seconds
    #[serde(default = "default_misfire_grace")]
    pub misfire_grace_seconds: usize,
}

fn default_max_concurrency() -> usize {
    1
}

fn default_timeout() -> usize {
    120
}

fn default_misfire_grace() -> usize {
    60
}

impl Default for JobRuntimeSpec {
    fn default() -> Self {
        Self {
            max_concurrency: default_max_concurrency(),
            timeout_seconds: default_timeout(),
            misfire_grace_seconds: default_misfire_grace(),
        }
    }
}

/// Request payload for agent-type cron jobs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CronJobRequest {
    /// Input for the agent (can be any JSON value)
    pub input: Value,

    /// Optional session ID (will be synced with target)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Optional user ID (will be synced with target)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// Task type for cron jobs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    Text,
    Agent,
}

impl Default for TaskType {
    fn default() -> Self {
        TaskType::Agent
    }
}

impl AsRef<str> for TaskType {
    fn as_ref(&self) -> &str {
        match self {
            TaskType::Text => "text",
            TaskType::Agent => "agent",
        }
    }
}

/// Cron job specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CronJobSpec {
    /// Unique job ID
    pub id: String,

    /// Human-readable job name
    pub name: String,

    /// Whether the job is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Schedule specification
    pub schedule: ScheduleSpec,

    /// Task type (text or agent)
    #[serde(default)]
    pub task_type: TaskType,

    /// Text content (for text-type jobs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Agent request (for agent-type jobs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<CronJobRequest>,

    /// Dispatch specification
    pub dispatch: DispatchSpec,

    /// Runtime specification
    #[serde(default)]
    pub runtime: JobRuntimeSpec,

    /// Additional metadata
    #[serde(default)]
    pub meta: HashMap<String, Value>,
}

fn default_enabled() -> bool {
    true
}

impl CronJobSpec {
    /// Validate the job specification
    pub fn validate(&self) -> Result<(), String> {
        // Validate cron expression
        ScheduleSpec::normalize_cron(&self.schedule.cron)?;

        // Validate task type fields
        if self.task_type == TaskType::Text {
            if self.text.as_ref().map_or(true, |t| t.trim().is_empty()) {
                return Err("task_type is text but text is empty".to_string());
            }
        } else if self.task_type == TaskType::Agent {
            if self.request.is_none() {
                return Err("task_type is agent but request is missing".to_string());
            }
        }

        Ok(())
    }
}

/// Jobs file structure (for JSON persistence)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobsFile {
    /// File format version
    #[serde(default = "default_version")]
    pub version: usize,

    /// List of jobs
    #[serde(default)]
    pub jobs: Vec<CronJobSpec>,
}

fn default_version() -> usize {
    1
}

impl Default for JobsFile {
    fn default() -> Self {
        Self {
            version: default_version(),
            jobs: Vec::new(),
        }
    }
}

/// Status of a cron job execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Success,
    Error,
    Running,
    Skipped,
}

/// State of a cron job (runtime information)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CronJobState {
    /// Next scheduled run time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Last run time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Status of the last run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<JobStatus>,

    /// Error message from the last run (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl Default for CronJobState {
    fn default() -> Self {
        Self {
            next_run_at: None,
            last_run_at: None,
            last_status: None,
            last_error: None,
        }
    }
}

/// View of a cron job (spec + state)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CronJobView {
    /// Job specification
    pub spec: CronJobSpec,

    /// Job state
    #[serde(default)]
    pub state: CronJobState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_schedule_spec_default() {
        let spec: ScheduleSpec = serde_json::from_value(json!({
            "cron": "0 0 * * *"
        }))
        .unwrap();

        assert_eq!(spec.r#type, "cron");
        assert_eq!(spec.timezone, "UTC");
    }

    #[test]
    fn test_schedule_spec_5_fields() {
        let result = ScheduleSpec::normalize_cron("0 0 * * *");
        assert_eq!(result.unwrap(), "0 0 0 * * *");
    }

    #[test]
    fn test_schedule_spec_4_fields() {
        let result = ScheduleSpec::normalize_cron("0 * * *");
        assert_eq!(result.unwrap(), "0 0 0 * * *");
    }

    #[test]
    fn test_schedule_spec_3_fields() {
        let result = ScheduleSpec::normalize_cron("* * *");
        assert_eq!(result.unwrap(), "0 0 0 * * *");
    }

    #[test]
    fn test_schedule_spec_invalid() {
        // 2 fields is invalid (should be 3, 4, or 5)
        let result = ScheduleSpec::normalize_cron("0 *");
        assert!(result.is_err());
    }

    #[test]
    fn test_dispatch_target() {
        let target = DispatchTarget {
            user_id: "user123".to_string(),
            session_id: "session456".to_string(),
        };

        let json = json!(&target);
        assert_eq!(json["user_id"], "user123");
        assert_eq!(json["session_id"], "session456");
    }

    #[test]
    fn test_dispatch_spec_default() {
        let spec: DispatchSpec = serde_json::from_value(json!({
            "target": {
                "user_id": "user123",
                "session_id": "session456"
            }
        }))
        .unwrap();

        assert_eq!(spec.r#type, "channel");
        assert_eq!(spec.channel, "console");
        assert_eq!(spec.mode, "stream");
    }

    #[test]
    fn test_job_runtime_spec_default() {
        let spec = JobRuntimeSpec::default();
        assert_eq!(spec.max_concurrency, 1);
        assert_eq!(spec.timeout_seconds, 120);
        assert_eq!(spec.misfire_grace_seconds, 60);
    }

    #[test]
    fn test_cron_job_request() {
        let request: CronJobRequest = serde_json::from_value(json!({
            "input": "Hello, world!"
        }))
        .unwrap();

        assert_eq!(request.input, json!("Hello, world!"));
        assert!(request.session_id.is_none());
        assert!(request.user_id.is_none());
    }

    #[test]
    fn test_cron_job_spec_text_type() {
        let spec: CronJobSpec = serde_json::from_value(json!({
            "id": "job1",
            "name": "Test Job",
            "schedule": {
                "cron": "0 0 * * *"
            },
            "task_type": "text",
            "text": "Hello from cron!",
            "dispatch": {
                "target": {
                    "user_id": "user123",
                    "session_id": "session456"
                }
            }
        }))
        .unwrap();

        assert_eq!(spec.id, "job1");
        assert_eq!(spec.name, "Test Job");
        assert_eq!(spec.task_type, TaskType::Text);
        assert_eq!(spec.text.as_deref(), Some("Hello from cron!"));
        assert!(spec.request.is_none());
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn test_cron_job_spec_agent_type() {
        let spec: CronJobSpec = serde_json::from_value(json!({
            "id": "job2",
            "name": "Agent Job",
            "schedule": {
                "cron": "0 0 * * *"
            },
            "task_type": "agent",
            "request": {
                "input": "What time is it?"
            },
            "dispatch": {
                "target": {
                    "user_id": "user123",
                    "session_id": "session456"
                }
            }
        }))
        .unwrap();

        assert_eq!(spec.task_type, TaskType::Agent);
        assert!(spec.text.is_none());
        assert!(spec.request.is_some());
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn test_cron_job_spec_validation_empty_text() {
        let spec: CronJobSpec = serde_json::from_value(json!({
            "id": "job1",
            "name": "Test Job",
            "schedule": {
                "cron": "0 0 * * *"
            },
            "task_type": "text",
            "text": "   ",
            "dispatch": {
                "target": {
                    "user_id": "user123",
                    "session_id": "session456"
                }
            }
        }))
        .unwrap();

        assert!(spec.validate().is_err());
    }

    #[test]
    fn test_cron_job_spec_validation_missing_request() {
        let spec: CronJobSpec = serde_json::from_value(json!({
            "id": "job2",
            "name": "Agent Job",
            "schedule": {
                "cron": "0 0 * * *"
            },
            "task_type": "agent",
            "dispatch": {
                "target": {
                    "user_id": "user123",
                    "session_id": "session456"
                }
            }
        }))
        .unwrap();

        assert!(spec.validate().is_err());
    }

    #[test]
    fn test_jobs_file_default() {
        let file = JobsFile::default();
        assert_eq!(file.version, 1);
        assert!(file.jobs.is_empty());
    }

    #[test]
    fn test_jobs_file_serialize() {
        let file = JobsFile {
            version: 1,
            jobs: vec![],
        };

        let json = json!(&file);
        assert_eq!(json["version"], 1);
        assert!(json["jobs"].is_array());
    }

    #[test]
    fn test_job_status() {
        let status: JobStatus = serde_json::from_str("\"success\"").unwrap();
        assert_eq!(status, JobStatus::Success);

        let status: JobStatus = serde_json::from_str("\"error\"").unwrap();
        assert_eq!(status, JobStatus::Error);

        let status: JobStatus = serde_json::from_str("\"running\"").unwrap();
        assert_eq!(status, JobStatus::Running);

        let status: JobStatus = serde_json::from_str("\"skipped\"").unwrap();
        assert_eq!(status, JobStatus::Skipped);
    }

    #[test]
    fn test_cron_job_state_default() {
        let state = CronJobState::default();
        assert!(state.next_run_at.is_none());
        assert!(state.last_run_at.is_none());
        assert!(state.last_status.is_none());
        assert!(state.last_error.is_none());
    }

    #[test]
    fn test_cron_job_view() {
        let spec: CronJobSpec = serde_json::from_value(json!({
            "id": "job1",
            "name": "Test Job",
            "schedule": {
                "cron": "0 0 * * *"
            },
            "task_type": "text",
            "text": "Hello!",
            "dispatch": {
                "target": {
                    "user_id": "user123",
                    "session_id": "session456"
                }
            }
        }))
        .unwrap();

        let view = CronJobView {
            spec,
            state: CronJobState::default(),
        };

        let json = json!(&view);
        assert!(json["spec"].is_object());
        assert!(json["state"].is_object());
    }

    #[test]
    fn test_task_type_as_ref() {
        assert_eq!(TaskType::Text.as_ref(), "text");
        assert_eq!(TaskType::Agent.as_ref(), "agent");
    }

    #[test]
    fn test_cron_job_spec_enabled_default() {
        let spec: CronJobSpec = serde_json::from_value(json!({
            "id": "job1",
            "name": "Test Job",
            "schedule": {
                "cron": "0 0 * * *"
            },
            "task_type": "text",
            "text": "Hello!",
            "dispatch": {
                "target": {
                    "user_id": "user123",
                    "session_id": "session456"
                }
            }
        }))
        .unwrap();

        assert!(spec.enabled);
    }

    #[test]
    fn test_cron_job_spec_disabled() {
        let spec: CronJobSpec = serde_json::from_value(json!({
            "id": "job1",
            "name": "Test Job",
            "enabled": false,
            "schedule": {
                "cron": "0 0 * * *"
            },
            "task_type": "text",
            "text": "Hello!",
            "dispatch": {
                "target": {
                    "user_id": "user123",
                    "session_id": "session456"
                }
            }
        }))
        .unwrap();

        assert!(!spec.enabled);
    }
}
