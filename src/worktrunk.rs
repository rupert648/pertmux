use anyhow::Result;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tracing::{info, warn};

/// Upper bound for a `wt` invocation. Without one, a `wt` that blocks on a
/// network fetch or a git credential helper (no controlling tty under systemd)
/// hangs the daemon command loop forever and the client action popup never
/// clears. `list` is local and quick; create/remove/merge may touch the network.
const WT_LIST_TIMEOUT: Duration = Duration::from_secs(30);
const WT_ACTION_TIMEOUT: Duration = Duration::from_secs(120);

/// Run a `wt` subcommand with stdin closed and a hard timeout. `kill_on_drop`
/// ensures the child is reaped if we time out (the future owning it is dropped).
async fn run_wt(args: &[&str], timeout: Duration) -> Result<std::process::Output> {
    let child = Command::new("wt")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(out) => Ok(out?),
        Err(_) => anyhow::bail!("wt {} timed out after {:?}", args.join(" "), timeout),
    }
}

/// Build a failure detail from a `wt` invocation. `wt` prints hook progress
/// banners (`◎ Running pre-switch ...`) to stderr, so the actual error is
/// usually the last non-empty line. Returns `(full, summary)`: `full` is the
/// whole stdout+stderr with newlines collapsed to ` | ` (single log line so
/// journald never hides the tail), `summary` is the last non-empty line for the
/// client-facing message.
fn wt_failure_detail(output: &std::process::Output) -> (String, String) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    summarize_wt_output(&stderr, &stdout, &output.status.to_string())
}

/// Pure core of `wt_failure_detail`, split out for testing. `fallback` is used
/// when no output line is available.
fn summarize_wt_output(stderr: &str, stdout: &str, fallback: &str) -> (String, String) {
    let combined: Vec<&str> = stderr
        .lines()
        .chain(stdout.lines())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let full = combined.join(" | ");
    let summary = combined
        .iter()
        .rev()
        .find(|l| l.starts_with('✗') || l.starts_with("error") || l.starts_with("fatal"))
        .or_else(|| combined.last())
        .map(|l| l.trim_start_matches('✗').trim().to_string())
        .unwrap_or_else(|| fallback.to_string());
    (full, summary)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct WtCommit {
    pub sha: String,
    pub short_sha: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct WtDiff {
    #[serde(default)]
    pub added: u64,
    #[serde(default)]
    pub deleted: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WtMain {
    #[serde(default)]
    pub ahead: u64,
    #[serde(default)]
    pub behind: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct WtWorktreeState {
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub detached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
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
    info!("fetch_worktrees: start (path={})", local_path);
    let t = std::time::Instant::now();

    let output = match run_wt(
        &["-C", local_path, "list", "--format=json"],
        WT_LIST_TIMEOUT,
    )
    .await
    {
        Ok(o) => o,
        Err(e)
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
        {
            info!("fetch_worktrees: wt binary not found, returning empty");
            return Ok(vec![]);
        }
        Err(e) => {
            warn!(
                "fetch_worktrees: command error after {:.2?}: {}",
                t.elapsed(),
                e
            );
            return Err(e);
        }
    };

    info!(
        "fetch_worktrees: wt exited (status={}) in {:.2?}",
        output.status,
        t.elapsed()
    );

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "fetch_worktrees: wt list failed ({}): {}",
            output.status,
            stderr.trim()
        );
        return Ok(vec![]);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        info!("fetch_worktrees: empty output, returning empty");
        return Ok(vec![]);
    }

    let all: Vec<WtWorktree> = serde_json::from_str(&stdout)?;
    let worktrees: Vec<WtWorktree> = all.into_iter().filter(|w| w.kind == "worktree").collect();
    info!(
        "fetch_worktrees: done — {} worktrees (elapsed={:.2?})",
        worktrees.len(),
        t.elapsed()
    );
    Ok(worktrees)
}

pub async fn create_worktree(local_path: &str, branch: &str, run_hooks: bool) -> Result<String> {
    info!(
        "create_worktree: start (path={}, branch={}, run_hooks={})",
        local_path, branch, run_hooks
    );
    let t = std::time::Instant::now();

    let mut args = vec![
        "-C", local_path, "switch", "--create", branch, "--no-cd", "-y",
    ];
    if !run_hooks {
        args.push("--no-hooks");
    }
    let output = run_wt(&args, WT_ACTION_TIMEOUT).await?;

    info!(
        "create_worktree: wt exited (status={}) in {:.2?}",
        output.status,
        t.elapsed()
    );

    if !output.status.success() {
        let (full, summary) = wt_failure_detail(&output);
        warn!("create_worktree: failed (branch={}): {}", branch, full);
        anyhow::bail!("{}", summary);
    }

    info!(
        "create_worktree: ok (branch={}, elapsed={:.2?})",
        branch,
        t.elapsed()
    );
    Ok(format!("Created worktree: {}", branch))
}

pub async fn remove_worktree(local_path: &str, branch: &str, run_hooks: bool) -> Result<String> {
    info!(
        "remove_worktree: start (path={}, branch={}, run_hooks={})",
        local_path, branch, run_hooks
    );
    let t = std::time::Instant::now();

    let mut args = vec![
        "-C",
        local_path,
        "remove",
        branch,
        "-y",
        "-f",
        "--foreground",
    ];
    if !run_hooks {
        args.push("--no-hooks");
    }
    let output = run_wt(&args, WT_ACTION_TIMEOUT).await?;

    info!(
        "remove_worktree: wt exited (status={}) in {:.2?}",
        output.status,
        t.elapsed()
    );

    if !output.status.success() {
        let (full, summary) = wt_failure_detail(&output);
        warn!("remove_worktree: failed (branch={}): {}", branch, full);
        anyhow::bail!("{}", summary);
    }

    info!(
        "remove_worktree: ok (branch={}, elapsed={:.2?})",
        branch,
        t.elapsed()
    );
    Ok(format!("Removed worktree: {}", branch))
}

pub async fn merge_worktree(worktree_path: &str, run_hooks: bool) -> Result<String> {
    info!(
        "merge_worktree: start (path={}, run_hooks={})",
        worktree_path, run_hooks
    );
    let t = std::time::Instant::now();

    let mut args = vec!["-C", worktree_path, "merge", "-y"];
    if !run_hooks {
        args.push("--no-hooks");
    }
    let output = run_wt(&args, WT_ACTION_TIMEOUT).await?;

    info!(
        "merge_worktree: wt exited (status={}) in {:.2?}",
        output.status,
        t.elapsed()
    );

    if !output.status.success() {
        let (full, summary) = wt_failure_detail(&output);
        warn!("merge_worktree: failed (path={}): {}", worktree_path, full);
        anyhow::bail!("{}", summary);
    }

    info!(
        "merge_worktree: ok (path={}, elapsed={:.2?})",
        worktree_path,
        t.elapsed()
    );
    Ok("Merged and cleaned up".to_string())
}

pub fn format_age(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "—".to_string();
    }
    let ts = match Timestamp::from_second(timestamp) {
        Ok(ts) => ts,
        Err(_) => return "—".to_string(),
    };
    let delta = (Timestamp::now().as_second() - ts.as_second()).max(0);

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

    #[test]
    fn test_summarize_wt_output_picks_error_line() {
        let stderr = "◎ Running pre-switch user:fetch\n  git fetch origin main\n✗ Branch test already exists\n↳ To switch to the existing branch, run without --create";
        let (full, summary) = summarize_wt_output(stderr, "", "exit status: 1");
        assert_eq!(summary, "Branch test already exists");
        assert!(full.contains("pre-switch"));
        assert!(full.contains(" | "));
    }

    #[test]
    fn test_summarize_wt_output_falls_back_to_last_line() {
        let (_, summary) = summarize_wt_output("some warning\nplain failure", "", "exit status: 1");
        assert_eq!(summary, "plain failure");
    }

    #[test]
    fn test_summarize_wt_output_empty_uses_fallback() {
        let (full, summary) = summarize_wt_output("", "", "exit status: 1");
        assert_eq!(summary, "exit status: 1");
        assert_eq!(full, "");
    }

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
        let filtered: Vec<WtWorktree> = all.into_iter().filter(|w| w.kind == "worktree").collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].branch.as_deref(), Some("main"));
    }

    #[test]
    fn test_format_age_just_now() {
        let now = Timestamp::now().as_second();
        assert_eq!(format_age(now), "just now");
        assert_eq!(format_age(now - 30), "just now");
    }

    #[test]
    fn test_format_age_minutes() {
        let now = Timestamp::now().as_second();
        assert_eq!(format_age(now - 300), "5m ago");
        assert_eq!(format_age(now - 3540), "59m ago");
    }

    #[test]
    fn test_format_age_hours() {
        let now = Timestamp::now().as_second();
        assert_eq!(format_age(now - 3600), "1h ago");
        assert_eq!(format_age(now - 7200), "2h ago");
    }

    #[test]
    fn test_format_age_days() {
        let now = Timestamp::now().as_second();
        assert_eq!(format_age(now - 86400), "1d ago");
        assert_eq!(format_age(now - 172800), "2d ago");
    }

    #[test]
    fn test_format_age_zero_timestamp() {
        assert_eq!(format_age(0), "\u{2014}");
        assert_eq!(format_age(-1), "\u{2014}");
    }
}
