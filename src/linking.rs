use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::forge_clients::types::MergeRequestSummary;
use crate::git::WorktreeInfo;
use crate::read_state::ReadStateDb;
use crate::types::AgentPane;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedMergeRequest {
    pub mr: MergeRequestSummary,
    pub worktree: Option<WorktreeInfo>,
    pub tmux_pane: Option<AgentPane>,
    pub has_new_activity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardState {
    pub linked_mrs: Vec<LinkedMergeRequest>,
}

pub fn link_all(
    mrs: &[MergeRequestSummary],
    worktrees: &[WorktreeInfo],
    panes: &[AgentPane],
    read_state: &ReadStateDb,
    project: &str,
) -> anyhow::Result<DashboardState> {
    let worktree_by_branch: HashMap<String, &WorktreeInfo> = worktrees
        .iter()
        .filter_map(|wt| wt.branch.as_ref().map(|branch| (branch.clone(), wt)))
        .collect();

    let pane_by_canonical_path: HashMap<PathBuf, &AgentPane> = panes
        .iter()
        .filter_map(|pane| {
            // Use the pre-computed canonical path (set once in tmux::make_agent_pane)
            // to avoid repeated fs::canonicalize syscalls per pane per tick.
            pane.canonical_path
                .as_deref()
                .map(PathBuf::from)
                .or_else(|| canonicalize_path(&pane.pane_path))
                .map(|path| (path, pane))
        })
        .collect();

    let mut linked_mrs = Vec::with_capacity(mrs.len());

    for mr in mrs {
        let worktree = worktree_by_branch.get(&mr.source_branch).copied();
        let pane = worktree
            .and_then(|wt| canonicalize_path(&wt.path))
            .and_then(|path| pane_by_canonical_path.get(&path).copied());

        let has_new_activity = read_state.has_new_activity(project, mr.iid, mr.user_notes_count)?;

        linked_mrs.push(LinkedMergeRequest {
            mr: mr.clone(),
            worktree: worktree.cloned(),
            tmux_pane: pane.cloned(),
            has_new_activity,
        });
    }

    Ok(DashboardState { linked_mrs })
}

fn canonicalize_path(path: &str) -> Option<PathBuf> {
    std::fs::canonicalize(Path::new(path)).ok()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::forge_clients::types::ForgeUser;
    use crate::types::PaneStatus;
    use jiff::Timestamp;

    fn test_db_path(test_name: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time should be after UNIX_EPOCH")
            .as_nanos();
        format!(
            "/tmp/test_linking_{}_{}_{}.db",
            test_name,
            std::process::id(),
            nanos
        )
    }

    fn make_mr(iid: u64, source_branch: &str, user_notes_count: u32) -> MergeRequestSummary {
        MergeRequestSummary {
            iid,
            title: "Test MR".to_string(),
            state: "opened".to_string(),
            source_branch: source_branch.to_string(),
            target_branch: "main".to_string(),
            author: ForgeUser {
                id: 1,
                username: "tester".to_string(),
                name: "Tester".to_string(),
            },
            draft: false,
            user_notes_count,
            web_url: "https://gitlab.example.com/team/project/-/merge_requests/1".to_string(),
            created_at: Timestamp::from_second(1_767_225_600).unwrap(),
            updated_at: Timestamp::from_second(1_767_225_600).unwrap(),
            detailed_merge_status: None,
            has_conflicts: None,
        }
    }

    fn make_worktree(path: &str, branch: Option<&str>) -> WorktreeInfo {
        WorktreeInfo {
            path: path.to_string(),
            branch: branch.map(ToString::to_string),
            head_commit: "abc123".to_string(),
            is_main: true,
            is_bare: false,
        }
    }

    fn make_pane(pane_id: &str, pane_path: &str) -> AgentPane {
        let canonical_path = std::fs::canonicalize(pane_path)
            .ok()
            .and_then(|p| p.to_str().map(String::from));
        AgentPane {
            pane_id: pane_id.to_string(),
            session_name: "s".to_string(),
            window_index: 0,
            pane_index: 0,
            pane_title: "pane".to_string(),
            pane_path: pane_path.to_string(),
            canonical_path,
            pane_pid: 1,
            pane_command: "opencode".to_string(),
            status: PaneStatus::Idle,
            db_session_title: None,
            agent: None,
            model: None,
            last_activity: None,
            status_changed_at: None,
            db_session_id: None,
            last_response: None,
        }
    }

    #[test]
    fn test_full_linking_chain() {
        let db_path = test_db_path("full_chain");
        let read_state = ReadStateDb::open(Some(&db_path)).expect("Should open test read-state DB");

        let mr = make_mr(42, "feat/linking", 3);
        let worktree = make_worktree("/tmp", Some("feat/linking"));
        let pane = make_pane("%1", "/tmp");

        let state = link_all(&[mr], &[worktree], &[pane], &read_state, "group/project")
            .expect("link_all should succeed");

        assert_eq!(state.linked_mrs.len(), 1);

        let linked = &state.linked_mrs[0];
        assert_eq!(linked.mr.iid, 42);
        assert!(linked.worktree.is_some());
        assert!(linked.tmux_pane.is_some());
        assert!(!linked.has_new_activity);

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_no_mrs_returns_empty() {
        let db_path = test_db_path("no_mrs");
        let read_state = ReadStateDb::open(Some(&db_path)).expect("Should open test read-state DB");

        let worktree = make_worktree("/tmp", Some("feat/solo"));
        let pane = make_pane("%2", "/tmp");

        let state = link_all(&[], &[worktree], &[pane], &read_state, "group/project")
            .expect("link_all should succeed");

        assert!(state.linked_mrs.is_empty());

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_empty_inputs() {
        let db_path = test_db_path("empty_inputs");
        let read_state = ReadStateDb::open(Some(&db_path)).expect("Should open test read-state DB");

        let state = link_all(&[], &[], &[], &read_state, "group/project")
            .expect("Empty inputs should not fail");

        assert!(state.linked_mrs.is_empty());

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_mr_without_matching_worktree() {
        let db_path = test_db_path("no_wt");
        let read_state = ReadStateDb::open(Some(&db_path)).expect("Should open test read-state DB");

        let mr = make_mr(99, "feat/no-worktree", 0);
        let worktree = make_worktree("/tmp", Some("feat/other-branch"));
        let pane = make_pane("%3", "/tmp");

        let state = link_all(&[mr], &[worktree], &[pane], &read_state, "group/project")
            .expect("link_all should succeed");

        assert_eq!(state.linked_mrs.len(), 1);
        let linked = &state.linked_mrs[0];
        assert_eq!(linked.mr.iid, 99);
        assert!(linked.worktree.is_none());
        assert!(linked.tmux_pane.is_none());

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_worktree_without_pane() {
        let db_path = test_db_path("wt_no_pane");
        let read_state = ReadStateDb::open(Some(&db_path)).expect("Should open test read-state DB");

        let mr = make_mr(50, "feat/wt-only", 2);
        let worktree = make_worktree("/tmp", Some("feat/wt-only"));
        let pane = make_pane("%4", "/var");

        let state = link_all(&[mr], &[worktree], &[pane], &read_state, "group/project")
            .expect("link_all should succeed");

        assert_eq!(state.linked_mrs.len(), 1);
        let linked = &state.linked_mrs[0];
        assert!(linked.worktree.is_some());
        assert!(linked.tmux_pane.is_none());

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_multiple_mrs_multiple_panes() {
        let db_path = test_db_path("multi");
        let read_state = ReadStateDb::open(Some(&db_path)).expect("Should open test read-state DB");

        let mr1 = make_mr(10, "feat/alpha", 0);
        let mr2 = make_mr(20, "feat/beta", 0);
        let wt1 = make_worktree("/tmp", Some("feat/alpha"));
        let wt2 = make_worktree("/var", Some("feat/beta"));
        let pane1 = make_pane("%10", "/tmp");
        let pane2 = make_pane("%20", "/var");

        let state = link_all(
            &[mr1, mr2],
            &[wt1, wt2],
            &[pane1, pane2],
            &read_state,
            "group/project",
        )
        .expect("link_all should succeed");

        assert_eq!(state.linked_mrs.len(), 2);

        assert_eq!(state.linked_mrs[0].mr.iid, 10);
        assert_eq!(
            state.linked_mrs[0].tmux_pane.as_ref().unwrap().pane_id,
            "%10"
        );
        assert_eq!(state.linked_mrs[1].mr.iid, 20);
        assert_eq!(
            state.linked_mrs[1].tmux_pane.as_ref().unwrap().pane_id,
            "%20"
        );

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_pane_path_not_canonicalizable() {
        let db_path = test_db_path("bad_path");
        let read_state = ReadStateDb::open(Some(&db_path)).expect("Should open test read-state DB");

        let mr = make_mr(77, "feat/ghost", 0);
        let worktree = make_worktree("/tmp", Some("feat/ghost"));
        let pane = make_pane("%99", "/nonexistent/path/xyz");

        let state = link_all(&[mr], &[worktree], &[pane], &read_state, "group/project")
            .expect("Should not panic on non-existent path");

        assert_eq!(state.linked_mrs.len(), 1);
        assert!(state.linked_mrs[0].worktree.is_some());
        assert!(state.linked_mrs[0].tmux_pane.is_none());

        let _ = std::fs::remove_file(db_path);
    }
}
