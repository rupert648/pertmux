use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum OpenCodeStatus {
    Idle,
    Busy,
    Retry { attempt: u32, message: String },
    Unknown,
}

impl OpenCodeStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::Busy => "busy",
            Self::Retry { .. } => "retrying",
            Self::Unknown => "no server",
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            Self::Idle => "○",
            Self::Busy => "●",
            Self::Retry { .. } => "⚠",
            Self::Unknown => "?",
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OpenCodePane {
    pub pane_id: String,
    pub session_name: String,
    pub window_index: u32,
    pub pane_index: u32,
    pub pane_title: String,
    pub pane_path: String,
    pub pane_pid: u32,

    pub api_port: Option<u16>,
    pub status: OpenCodeStatus,

    pub db_session_title: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub last_activity: Option<i64>,
}

impl OpenCodePane {
    pub fn display_title(&self) -> &str {
        if let Some(ref title) = self.db_session_title {
            title.as_str()
        } else {
            self.pane_title
                .strip_prefix("OC | ")
                .unwrap_or(&self.pane_title)
        }
    }

    pub fn display_model(&self) -> &str {
        self.model.as_deref().unwrap_or("unknown")
    }

    pub fn display_agent(&self) -> &str {
        self.agent.as_deref().unwrap_or("unknown")
    }

    pub fn time_ago(&self) -> Option<String> {
        let ts = self.last_activity?;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis() as i64;
        let elapsed_secs = (now_ms - ts) / 1000;
        if elapsed_secs < 0 {
            return None;
        }
        let s = elapsed_secs;
        Some(if s < 60 {
            format!("{}s ago", s)
        } else if s < 3600 {
            format!("{}m ago", s / 60)
        } else if s < 86400 {
            format!("{}h ago", s / 3600)
        } else {
            format!("{}d ago", s / 86400)
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct SessionStatus {
    #[serde(rename = "type")]
    pub status_type: String,
    pub attempt: Option<u32>,
    pub message: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub healthy: bool,
}

pub type SessionStatusMap = HashMap<String, SessionStatus>;
