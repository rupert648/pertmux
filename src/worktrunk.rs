use anyhow::Result;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;

#[derive(Debug, Clone, Deserialize)]
pub struct WtCommit {
    pub sha: String,
    pub short_sha: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub timestamp: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WtDiff {
    #[serde(default)]
    pub added: u64,
    #[serde(default)]
    pub deleted: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WtWorkingTree {
    #[serde(default)]
    pub staged: bool,
    #[serde(default)]
    pub modified: bool,
    #[serde(default)]
    pub untracked: bool,
    #[serde(default)]
    pub renamed: bool,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub diff: Option<WtDiff>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WtMain {
    #[serde(default)]
    pub ahead: u64,
    #[serde(default)]
    pub behind: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WtRemote {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub ahead: u64,
    #[serde(default)]
    pub behind: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WtWorktreeState {
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub detached: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WtWorktree {
    pub branch: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    pub kind: String,
    pub commit: WtCommit,
    #[serde(default)]
    pub working_tree: Option<WtWorkingTree>,
    #[serde(default)]
    pub main_state: Option<String>,
    #[serde(default)]
    pub main: Option<WtMain>,
    #[serde(default)]
    pub remote: Option<WtRemote>,
    #[serde(default)]
    pub worktree: Option<WtWorktreeState>,
    #[serde(default)]
    pub is_main: bool,
    #[serde(default)]
    pub is_current: bool,
    #[serde(default)]
    pub is_previous: bool,
    #[serde(default)]
    pub symbols: Option<String>,
}

pub async fn fetch_worktrees(local_path: &str) -> Result<Vec<WtWorktree>> {
    let output = match Command::new("wt")
        .args(["-C", local_path, "list", "--format=json"])
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(vec![]);
        }
        Err(e) => return Err(e.into()),
    };

    if !output.status.success() {
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(vec![]);
    }

    let all: Vec<WtWorktree> = serde_json::from_str(&stdout)?;
    Ok(all.into_iter().filter(|w| w.kind == "worktree").collect())
}

pub fn format_age(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "—".to_string();
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let delta = (now - timestamp).max(0);

    if delta < 60 {
        "just now".to_string()
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_WT_JSON: &str = r#"[
        {
            "branch": "master",
            "path": "/Users/rupert/project",
            "kind": "worktree",
            "commit": {
                "sha": "180e97b75cabe4cb37a38ae0a79de69887e41841",
                "short_sha": "180e97b",
                "message": "Merge branch 'rupert/profile' into 'master'",
                "timestamp": 1772623959
            },
            "working_tree": {
                "staged": false,
                "modified": false,
                "untracked": false,
                "renamed": false,
                "deleted": false,
                "diff": { "added": 0, "deleted": 0 }
            },
            "main_state": "is_main",
            "remote": {
                "name": "origin",
                "branch": "master",
                "ahead": 0,
                "behind": 0
            },
            "worktree": { "detached": false },
            "is_main": true,
            "is_current": true,
            "is_previous": false,
            "statusline": "master  ^|",
            "symbols": "^|"
        },
        {
            "branch": "rupert/refactor-stage-1",
            "path": "/Users/rupert/project-worktrees/refactor",
            "kind": "worktree",
            "commit": {
                "sha": "b2caff31aa5e3eca61d9d43cec19f9da89e1ee2d",
                "short_sha": "b2caff3",
                "message": "NC-7630: Use static refs through flow handlers",
                "timestamp": 1772624800
            },
            "working_tree": {
                "staged": false,
                "modified": false,
                "untracked": false,
                "renamed": false,
                "deleted": false,
                "diff": { "added": 0, "deleted": 0 }
            },
            "main_state": "diverged",
            "main": { "ahead": 2, "behind": 12 },
            "remote": {
                "name": "origin",
                "branch": "rupert/refactor-stage-1",
                "ahead": 0,
                "behind": 0
            },
            "worktree": {
                "state": "branch_worktree_mismatch",
                "detached": false
            },
            "is_main": false,
            "is_current": false,
            "is_previous": false,
            "statusline": "rupert/refactor-stage-1  ⚑↕|  ↑2 ↓12",
            "symbols": "↕|⚑"
        }
    ]"#;

    #[test]
    fn test_parse_real_wt_json() {
        let worktrees: Vec<WtWorktree> = serde_json::from_str(REAL_WT_JSON).unwrap();
        assert_eq!(worktrees.len(), 2);

        let main = &worktrees[0];
        assert_eq!(main.branch.as_deref(), Some("master"));
        assert_eq!(main.path.as_deref(), Some("/Users/rupert/project"));
        assert_eq!(main.kind, "worktree");
        assert!(main.is_main);
        assert!(main.is_current);
        assert_eq!(main.commit.short_sha, "180e97b");
        assert_eq!(main.main_state.as_deref(), Some("is_main"));
        assert!(main.main.is_none());
        assert_eq!(main.symbols.as_deref(), Some("^|"));

        let wt = &worktrees[1];
        assert_eq!(wt.branch.as_deref(), Some("rupert/refactor-stage-1"));
        assert!(!wt.is_main);
        let m = wt.main.as_ref().unwrap();
        assert_eq!(m.ahead, 2);
        assert_eq!(m.behind, 12);
        assert_eq!(
            wt.worktree.as_ref().unwrap().state.as_deref(),
            Some("branch_worktree_mismatch")
        );
    }

    #[test]
    fn test_parse_minimal_wt_json() {
        let json = r#"[{
            "branch": null,
            "kind": "worktree",
            "commit": { "sha": "abc", "short_sha": "abc" },
            "is_main": false,
            "is_current": false,
            "is_previous": false
        }]"#;
        let worktrees: Vec<WtWorktree> = serde_json::from_str(json).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].branch.is_none());
        assert!(worktrees[0].path.is_none());
        assert!(worktrees[0].working_tree.is_none());
        assert!(worktrees[0].main.is_none());
        assert!(worktrees[0].remote.is_none());
        assert!(worktrees[0].symbols.is_none());
    }

    #[test]
    fn test_parse_unknown_fields_ignored() {
        let json = r#"[{
            "branch": "feat/test",
            "path": "/tmp/test",
            "kind": "worktree",
            "commit": { "sha": "abc", "short_sha": "abc", "message": "test", "timestamp": 0 },
            "is_main": false,
            "is_current": false,
            "is_previous": false,
            "new_future_field": true,
            "another_field": { "nested": 42 }
        }]"#;
        let worktrees: Vec<WtWorktree> = serde_json::from_str(json).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].branch.as_deref(), Some("feat/test"));
    }

    #[test]
    fn test_filter_kind_worktree_only() {
        let json = r#"[
            { "branch": "main", "path": "/tmp", "kind": "worktree", "commit": { "sha": "a", "short_sha": "a" }, "is_main": true, "is_current": false, "is_previous": false },
            { "branch": "feat/old", "kind": "branch", "commit": { "sha": "b", "short_sha": "b" }, "is_main": false, "is_current": false, "is_previous": false }
        ]"#;
        let all: Vec<WtWorktree> = serde_json::from_str(json).unwrap();
        let filtered: Vec<WtWorktree> =
            all.into_iter().filter(|w| w.kind == "worktree").collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].branch.as_deref(), Some("main"));
    }

    #[test]
    fn test_format_age_just_now() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert_eq!(format_age(now), "just now");
        assert_eq!(format_age(now - 30), "just now");
    }

    #[test]
    fn test_format_age_minutes() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert_eq!(format_age(now - 300), "5m ago");
        assert_eq!(format_age(now - 3540), "59m ago");
    }

    #[test]
    fn test_format_age_hours() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert_eq!(format_age(now - 3600), "1h ago");
        assert_eq!(format_age(now - 7200), "2h ago");
    }

    #[test]
    fn test_format_age_days() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert_eq!(format_age(now - 86400), "1d ago");
        assert_eq!(format_age(now - 172800), "2d ago");
    }

    #[test]
    fn test_format_age_zero_timestamp() {
        assert_eq!(format_age(0), "\u{2014}");
        assert_eq!(format_age(-1), "\u{2014}");
    }
}
