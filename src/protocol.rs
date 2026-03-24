use crate::agent_changes::AgentChange;
use crate::config::{AgentActionConfig, KeybindingsConfig, ProjectForge};
use crate::forge_clients::types::{
    MergeRequestDetail, MergeRequestThread, PipelineJob, UserMrSummary,
};
use crate::linking::DashboardState;
use crate::mr_changes::MrChange;
use crate::types::{AgentPane, SessionDetail};
use crate::worktrunk::WtWorktree;
use serde::{Deserialize, Serialize};

/// Navigation target carried by an activity entry.
/// Used by the activity feed popup to jump to the relevant tmux pane or MR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityTarget {
    /// Switch to a specific tmux pane (agent activities).
    Pane { pane_id: String, pane_path: String },
    /// Navigate to an MR in a configured project (forge activities).
    MergeRequest { project_name: String, iid: u64 },
}

/// The kind of activity, used to assign display color in the feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityKind {
    /// Agent started working (Busy)
    AgentBusy,
    /// Agent finished / became idle
    AgentIdle,
    /// Agent entered retry state
    AgentRetry,
    /// MR pipeline failed
    MrPipelineFailed,
    /// MR pipeline succeeded
    MrPipelineSucceeded,
    /// New MR discussions
    MrNewDiscussions,
    /// MR approved
    MrApproved,
}

/// A single entry in the activity feed, persisted in the daemon.
/// Uses Unix seconds (`received_at_secs`) so it can be serialized
/// across the IPC boundary and survive client reconnects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// Short display label (last path component of pane_path, or project name)
    pub label: String,
    /// Human-readable description of what changed
    pub message: String,
    pub kind: ActivityKind,
    /// Unix timestamp (seconds since UNIX epoch) when the daemon recorded this event
    pub received_at_secs: u64,
    /// Navigation target — used by the activity feed popup to jump to the relevant item.
    #[serde(default)]
    pub target: Option<ActivityTarget>,
}

impl From<&crate::agent_changes::AgentChange> for ActivityEntry {
    fn from(change: &crate::agent_changes::AgentChange) -> Self {
        use crate::agent_changes::AgentChangeType;
        let label = change
            .pane_path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&change.pane_path)
            .to_string();
        let (message, kind) = match change.change_type {
            AgentChangeType::Busy => ("working".to_string(), ActivityKind::AgentBusy),
            AgentChangeType::Idle => ("finished".to_string(), ActivityKind::AgentIdle),
            AgentChangeType::Retry => ("retrying".to_string(), ActivityKind::AgentRetry),
        };
        ActivityEntry {
            label,
            message,
            kind,
            received_at_secs: jiff::Timestamp::now().as_second() as u64,
            target: Some(ActivityTarget::Pane {
                pane_id: change.pane_id.clone(),
                pane_path: change.pane_path.clone(),
            }),
        }
    }
}

impl From<&crate::mr_changes::MrChange> for ActivityEntry {
    fn from(change: &crate::mr_changes::MrChange) -> Self {
        use crate::mr_changes::MrChangeType;
        let (message, kind) = match &change.change_type {
            MrChangeType::PipelineFailed => (
                format!("!{} pipeline failed", change.mr_iid),
                ActivityKind::MrPipelineFailed,
            ),
            MrChangeType::PipelineSucceeded => (
                format!("!{} pipeline ok", change.mr_iid),
                ActivityKind::MrPipelineSucceeded,
            ),
            MrChangeType::NewDiscussions(n) => (
                format!(
                    "!{} {} new comment{}",
                    change.mr_iid,
                    n,
                    if *n == 1 { "" } else { "s" }
                ),
                ActivityKind::MrNewDiscussions,
            ),
            MrChangeType::Approved => (
                format!("!{} approved", change.mr_iid),
                ActivityKind::MrApproved,
            ),
        };
        ActivityEntry {
            label: change.project_name.clone(),
            message,
            kind,
            received_at_secs: jiff::Timestamp::now().as_second() as u64,
            target: Some(ActivityTarget::MergeRequest {
                project_name: change.project_name.clone(),
                iid: change.mr_iid,
            }),
        }
    }
}

#[allow(dead_code)]
pub const PROTOCOL_VERSION: u32 = 2;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub name: String,
    pub source: ProjectForge,
    pub project_path: String,
    pub local_path: String,
    pub dashboard: DashboardState,
    pub cached_worktrees: Vec<WtWorktree>,
    pub cached_mr_detail: Option<MergeRequestDetail>,
    pub cached_pipeline_jobs: Vec<PipelineJob>,
    #[serde(default)]
    pub cached_threads: Vec<MergeRequestThread>,
    #[serde(default)]
    pub cached_threads_iid: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalMrEntry {
    pub mr: UserMrSummary,
    pub forge: ProjectForge,
    pub configured_project: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSnapshot {
    pub projects: Vec<ProjectSnapshot>,
    pub panes: Vec<AgentPane>,
    pub groups: Vec<(String, Vec<usize>)>,
    pub detail: Option<SessionDetail>,
    pub error: Option<String>,
    pub seconds_since_refresh: u64,
    #[serde(default)]
    pub default_agent_command: Option<String>,
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
    #[serde(default)]
    pub pending_changes: Vec<MrChange>,
    #[serde(default)]
    pub agent_actions: Vec<AgentActionConfig>,
    #[serde(default)]
    pub pending_agent_changes: Vec<AgentChange>,
    #[serde(default)]
    pub global_mrs: Vec<GlobalMrEntry>,
    /// Full activity feed history, managed entirely by the daemon.
    /// Persists between client connects — a fresh `pertmux connect` will
    /// see all events that happened since the daemon started.
    #[serde(default)]
    pub activity_feed: Vec<ActivityEntry>,
}

impl PartialEq for DashboardSnapshot {
    fn eq(&self, other: &Self) -> bool {
        match (serde_json::to_value(self), serde_json::to_value(other)) {
            (Ok(left), Ok(right)) => left == right,
            _ => false,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMsg {
    Handshake {
        version: u32,
    },
    Refresh,
    SelectMr {
        project_idx: usize,
        mr_iid: u64,
    },
    CreateWorktree {
        project_idx: usize,
        branch: String,
    },
    RemoveWorktree {
        project_idx: usize,
        branch: String,
    },
    MergeWorktree {
        project_idx: usize,
        worktree_path: String,
    },
    AgentAction {
        pane_pid: u32,
        session_id: String,
        prompt: String,
    },
    Stop,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonMsg {
    HandshakeAck { version: u32 },
    Snapshot(Box<DashboardSnapshot>),
    ActionResult { ok: bool, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge_clients::types::{ForgeUser, MergeRequestSummary};
    use crate::git::WorktreeInfo;
    use crate::linking::LinkedMergeRequest;
    use crate::types::PaneStatus;
    use crate::worktrunk::WtCommit;
    use jiff::Timestamp;

    #[test]
    fn dashboard_snapshot_round_trip_json() {
        let mr = MergeRequestSummary {
            iid: 42,
            title: "feat: add daemon protocol".to_string(),
            state: "opened".to_string(),
            source_branch: "feat/protocol".to_string(),
            target_branch: "main".to_string(),
            author: ForgeUser {
                id: 1,
                username: "rupert".to_string(),
                name: "Rupert".to_string(),
            },
            draft: false,
            user_notes_count: 1,
            web_url: "https://gitlab.example.com/team/pertmux/-/merge_requests/42".to_string(),
            created_at: "2026-03-01T00:00:00.000Z".parse().unwrap(),
            updated_at: "2026-03-01T00:00:00.000Z".parse().unwrap(),
            detailed_merge_status: Some("mergeable".to_string()),
            has_conflicts: Some(false),
        };

        let linked_mr = LinkedMergeRequest {
            mr,
            worktree: Some(WorktreeInfo {
                path: "/tmp/pertmux-worktree".to_string(),
                branch: Some("feat/protocol".to_string()),
                head_commit: "abc123".to_string(),
                is_main: false,
                is_bare: false,
            }),
            tmux_pane: None,
            has_new_activity: true,
        };

        let pane = AgentPane {
            pane_id: "%1".to_string(),
            session_name: "pertmux".to_string(),
            window_index: 0,
            pane_index: 0,
            pane_title: "OC | protocol".to_string(),
            pane_path: "/tmp/pertmux-worktree".to_string(),
            pane_pid: 1234,
            pane_command: "opencode".to_string(),
            status: PaneStatus::Idle,
            db_session_title: Some("Protocol work".to_string()),
            agent: Some("opencode".to_string()),
            model: Some("gpt-5".to_string()),
            last_activity: Some(Timestamp::from_millisecond(1_762_000_000_000).unwrap()),
            status_changed_at: None,
            db_session_id: Some("sess-1".to_string()),
            last_response: Some("done".to_string()),
        };

        let snapshot = DashboardSnapshot {
            projects: vec![ProjectSnapshot {
                name: "pertmux".to_string(),
                source: ProjectForge::Gitlab,
                project_path: "team/pertmux".to_string(),
                local_path: "/tmp/pertmux".to_string(),
                dashboard: DashboardState {
                    linked_mrs: vec![linked_mr],
                },
                cached_worktrees: vec![WtWorktree {
                    branch: Some("feat/protocol".to_string()),
                    path: Some("/tmp/pertmux-worktree".to_string()),
                    kind: "worktree".to_string(),
                    commit: WtCommit {
                        sha: "abc123".to_string(),
                        short_sha: "abc123".to_string(),
                        message: "protocol".to_string(),
                        timestamp: 1_762_000_000,
                    },
                    working_tree: None,
                    main_state: None,
                    main: None,
                    remote: None,
                    worktree: None,
                    is_main: false,
                    is_current: true,
                    is_previous: false,
                    symbols: Some("|".to_string()),
                }],
                cached_mr_detail: None,
                cached_pipeline_jobs: vec![],
                cached_threads: vec![],
                cached_threads_iid: None,
            }],
            panes: vec![pane],
            groups: vec![("pertmux".to_string(), vec![0])],
            detail: None,
            error: None,
            seconds_since_refresh: 2,
            default_agent_command: None,
            keybindings: KeybindingsConfig::default(),
            pending_changes: vec![],
            agent_actions: vec![],
            pending_agent_changes: vec![],
            global_mrs: vec![],
            activity_feed: vec![],
        };

        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let decoded: DashboardSnapshot = serde_json::from_str(&json).expect("deserialize snapshot");

        assert_eq!(decoded, snapshot);
    }
}
