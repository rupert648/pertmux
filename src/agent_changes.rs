use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents a single agent status transition event.
/// Sent from daemon to client in DashboardSnapshot.pending_agent_changes.
/// The client timestamps entries on arrival (Instant::now()) for recency tracking.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentChange {
    /// Tmux pane ID (e.g. "%42") — unique identifier for the pane
    pub pane_id: String,
    /// Working directory of the pane (for display label)
    pub pane_path: String,
    /// Tmux session name (for display context)
    pub session_name: String,
    pub change_type: AgentChangeType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentChangeType {
    /// Agent started working (transitioned to Busy)
    Busy,
    /// Agent finished / became idle (transitioned from Busy/Retry to Idle)
    Idle,
    /// Agent entered retry state
    Retry,
}

impl fmt::Display for AgentChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let action = match &self.change_type {
            AgentChangeType::Busy => "started working",
            AgentChangeType::Idle => "finished",
            AgentChangeType::Retry => "retrying",
        };
        // Show the last path component as the label (e.g. "my-worktree")
        let label = self
            .pane_path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&self.pane_path);
        write!(f, "{}: {}", label, action)
    }
}
