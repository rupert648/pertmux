use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaneStatus {
    Idle,
    Busy,
    Retry { attempt: u32, message: String },
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AgentPane {
    pub pane_id: String,
    pub session_name: String,
    pub window_index: u32,
    pub pane_index: u32,
    pub pane_title: String,
    pub pane_path: String,
    /// Pre-resolved canonical path (symlinks resolved). Computed once when the
    /// pane is discovered and reused by linking to avoid repeated `fs::canonicalize`
    /// syscalls on every tick.
    #[serde(default)]
    pub canonical_path: Option<String>,
    pub pane_pid: u32,
    pub pane_command: String,

    pub status: PaneStatus,

    pub db_session_title: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub last_activity: Option<Timestamp>,
    #[serde(default)]
    pub status_changed_at: Option<Timestamp>,
    pub db_session_id: Option<String>,
    pub last_response: Option<String>,
}

impl AgentPane {
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
        let now = Timestamp::now();
        now.since(ts).ok()?;
        let elapsed_secs = now.as_second() - ts.as_second();
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

/// Detailed information about a session, shown in the detail panel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SessionDetail {
    pub session_id: String,
    pub title: String,
    pub directory: String,
    pub message_count: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub session_created: Option<Timestamp>,
    pub session_updated: Option<Timestamp>,
    pub summary_files: Option<u32>,
    pub summary_additions: Option<u32>,
    pub summary_deletions: Option<u32>,
    pub messages: Vec<MessageSummary>,
    pub todos: Vec<TodoItem>,
}

/// A single message turn for the timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MessageSummary {
    pub role: String,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub output_tokens: u64,
    pub timestamp: Timestamp,
    pub text_preview: Option<String>,
}

/// A todo item from the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct TodoItem {
    pub content: String,
    pub status: String,
    pub priority: String,
}
