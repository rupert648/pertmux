use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::git::WorktreeInfo;
use crate::gitlab::types::MergeRequestSummary;
use crate::read_state::ReadStateDb;
use crate::types::AgentPane;

#[derive(Debug, Clone)]
pub struct LinkedMergeRequest {
    pub mr: MergeRequestSummary,
    pub worktree: Option<WorktreeInfo>,
    pub tmux_pane: Option<AgentPane>,
    pub has_new_activity: bool,
}

#[derive(Debug, Clone)]
pub struct UnlinkedInstance {
    pub pane: AgentPane,
    pub worktree: Option<WorktreeInfo>,
    pub branch: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DashboardState {
    pub linked_mrs: Vec<LinkedMergeRequest>,
    pub unlinked_instances: Vec<UnlinkedInstance>,
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
        .filter_map(|pane| canonicalize_path(&pane.pane_path).map(|path| (path, pane)))
        .collect();

    let worktree_by_canonical_path: HashMap<PathBuf, &WorktreeInfo> = worktrees
        .iter()
        .filter_map(|wt| canonicalize_path(&wt.path).map(|path| (path, wt)))
        .collect();

    let mut matched_pane_ids = HashSet::new();
    let mut linked_mrs = Vec::with_capacity(mrs.len());

    for mr in mrs {
        let worktree = worktree_by_branch.get(&mr.source_branch).copied();
        let pane = worktree
            .and_then(|wt| canonicalize_path(&wt.path))
            .and_then(|path| pane_by_canonical_path.get(&path).copied());

        if let Some(found_pane) = pane {
            matched_pane_ids.insert(found_pane.pane_id.clone());
        }

        let has_new_activity = read_state.has_new_activity(project, mr.iid, mr.user_notes_count)?;

        linked_mrs.push(LinkedMergeRequest {
            mr: mr.clone(),
            worktree: worktree.cloned(),
            tmux_pane: pane.cloned(),
            has_new_activity,
        });
    }

    let mut unlinked_instances = Vec::new();
    for pane in panes {
        if matched_pane_ids.contains(&pane.pane_id) {
            continue;
        }

        let worktree = canonicalize_path(&pane.pane_path)
            .and_then(|path| worktree_by_canonical_path.get(&path).copied());

        unlinked_instances.push(UnlinkedInstance {
            pane: pane.clone(),
            worktree: worktree.cloned(),
            branch: worktree.and_then(|wt| wt.branch.clone()),
        });
    }

    Ok(DashboardState {
        linked_mrs,
        unlinked_instances,
    })
}

fn canonicalize_path(path: &str) -> Option<PathBuf> {
    std::fs::canonicalize(Path::new(path)).ok()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::gitlab::types::GitLabUser;
    use crate::types::PaneStatus;

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
            author: GitLabUser {
                id: 1,
                username: "tester".to_string(),
                name: "Tester".to_string(),
            },
            draft: false,
            user_notes_count,
            web_url: "https://gitlab.example.com/team/project/-/merge_requests/1".to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
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
        AgentPane {
            pane_id: pane_id.to_string(),
            session_name: "s".to_string(),
            window_index: 0,
            pane_index: 0,
            pane_title: "pane".to_string(),
            pane_path: pane_path.to_string(),
            pane_pid: 1,
            pane_command: "opencode".to_string(),
            status: PaneStatus::Idle,
            db_session_title: None,
            agent: None,
            model: None,
            last_activity: None,
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
        assert_eq!(state.unlinked_instances.len(), 0);

        let linked = &state.linked_mrs[0];
        assert_eq!(linked.mr.iid, 42);
        assert!(linked.worktree.is_some());
        assert!(linked.tmux_pane.is_some());
        assert!(!linked.has_new_activity);

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_unlinked_pane() {
        let db_path = test_db_path("unlinked_pane");
        let read_state = ReadStateDb::open(Some(&db_path)).expect("Should open test read-state DB");

        let worktree = make_worktree("/tmp", Some("feat/solo"));
        let pane = make_pane("%2", "/tmp");

        let state = link_all(&[], &[worktree], &[pane], &read_state, "group/project")
            .expect("link_all should succeed");

        assert_eq!(state.linked_mrs.len(), 0);
        assert_eq!(state.unlinked_instances.len(), 1);

        let unlinked = &state.unlinked_instances[0];
        assert_eq!(unlinked.pane.pane_id, "%2");
        assert!(unlinked.worktree.is_some());
        assert_eq!(unlinked.branch.as_deref(), Some("feat/solo"));

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_empty_inputs() {
        let db_path = test_db_path("empty_inputs");
        let read_state = ReadStateDb::open(Some(&db_path)).expect("Should open test read-state DB");

        let state = link_all(&[], &[], &[], &read_state, "group/project")
            .expect("Empty inputs should not fail");

        assert!(state.linked_mrs.is_empty());
        assert!(state.unlinked_instances.is_empty());

        let _ = std::fs::remove_file(db_path);
    }
}
