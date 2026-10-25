use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::worktrunk::WtWorktree;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct WorktreeInfo {
    pub path: String,           // Absolute, canonicalized filesystem path
    pub branch: Option<String>, // Branch name (None if detached HEAD or bare)
    pub head_commit: String,    // HEAD commit hash
    pub is_main: bool,          // True if this is the main (first) worktree
    pub is_bare: bool,          // True if bare repository
}

/// Discover all git worktrees for the repository at `repo_path`.
/// Uses `git -C <repo_path> worktree list --porcelain` and parses the output.
pub async fn discover_worktrees(repo_path: &str) -> Result<Vec<WorktreeInfo>> {
    let output = Command::new("git")
        .args(["-C", repo_path, "worktree", "list", "--porcelain"])
        .output()
        .await
        .context("Failed to run git worktree list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git worktree list failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_worktree_output(&stdout)
}

/// Build the linking representation directly from a successful Worktrunk listing.
pub fn worktree_info_from_worktrunk(worktrees: &[WtWorktree]) -> Vec<WorktreeInfo> {
    worktrees
        .iter()
        .filter_map(|worktree| {
            let path = worktree.path.as_deref()?;
            let path = std::fs::canonicalize(path)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string());
            let detached = worktree
                .worktree
                .as_ref()
                .is_some_and(|state| state.detached);

            Some(WorktreeInfo {
                path,
                branch: if detached {
                    None
                } else {
                    worktree.branch.clone()
                },
                head_commit: worktree.commit.sha.clone(),
                is_main: worktree.is_main,
                is_bare: false,
            })
        })
        .collect()
}

/// Parse the porcelain output of `git worktree list --porcelain`.
/// Output format:
///   worktree /absolute/path
///   HEAD <commit-hash>
///   branch refs/heads/<name>    ← or "detached" or "bare"
///   <blank line separates worktrees>
fn parse_worktree_output(output: &str) -> Result<Vec<WorktreeInfo>> {
    let mut worktrees = Vec::new();
    let blocks: Vec<&str> = output.split("\n\n").collect();

    for (i, block) in blocks.iter().enumerate() {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let mut path: Option<String> = None;
        let mut head_commit = String::new();
        let mut branch: Option<String> = None;
        let mut is_bare = false;

        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                // Canonicalize path to resolve symlinks
                let canonical = std::fs::canonicalize(p)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| p.to_string());
                path = Some(canonical);
            } else if let Some(h) = line.strip_prefix("HEAD ") {
                head_commit = h.to_string();
            } else if let Some(b) = line.strip_prefix("branch ") {
                // "branch refs/heads/main" → "main"
                branch = b.strip_prefix("refs/heads/").map(|s| s.to_string());
            } else if line == "bare" {
                is_bare = true;
            }
            // "detached" line means branch stays None — that's the correct behavior
        }

        if let Some(path) = path {
            worktrees.push(WorktreeInfo {
                path,
                branch,
                head_commit,
                is_main: i == 0, // First block is always the main worktree
                is_bare,
            });
        }
    }

    Ok(worktrees)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_worktree() {
        let output = "worktree /home/user/project\nHEAD abc123\nbranch refs/heads/main\n\n";
        let result = parse_worktree_output(output).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].branch, Some("main".to_string()));
        assert_eq!(result[0].head_commit, "abc123");
        assert!(result[0].is_main);
        assert!(!result[0].is_bare);
    }

    #[test]
    fn test_parse_multiple_worktrees() {
        let output = concat!(
            "worktree /home/user/project\n",
            "HEAD abc123\n",
            "branch refs/heads/main\n",
            "\n",
            "worktree /home/user/feature-branch\n",
            "HEAD def456\n",
            "branch refs/heads/feat/login\n",
            "\n"
        );
        let result = parse_worktree_output(output).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].is_main);
        assert!(!result[1].is_main);
        assert_eq!(result[1].branch, Some("feat/login".to_string()));
    }

    #[test]
    fn test_parse_detached_head() {
        let output = "worktree /home/user/detached\nHEAD abc123\ndetached\n\n";
        let result = parse_worktree_output(output).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].branch.is_none());
    }

    #[test]
    fn test_parse_bare_repo() {
        let output =
            "worktree /home/user/project.git\nHEAD abc123\nbranch refs/heads/main\nbare\n\n";
        let result = parse_worktree_output(output).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].is_bare);
    }

    #[test]
    fn test_parse_empty_output() {
        let result = parse_worktree_output("").unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_non_git_dir_returns_error() {
        let result = discover_worktrees("/tmp").await;
        assert!(result.is_err(), "Expected error for non-git directory");
        let err = result.unwrap_err().to_string();
        // Error should mention the failure clearly
        assert!(
            err.contains("not a git repo") || err.contains("git worktree list failed"),
            "Expected descriptive error, got: {}",
            err
        );
    }

    #[test]
    fn worktrunk_conversion_canonicalizes_paths_and_preserves_detached_state() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pertmux-worktrunk-conversion-{}-{nonce}",
            std::process::id()
        ));
        let checkout = root.join("checkout");
        std::fs::create_dir_all(&checkout).unwrap();
        let non_canonical = checkout.join("..").join("checkout");
        let worktree = WtWorktree {
            branch: Some("branch-ignored-for-detached-head".to_string()),
            path: Some(non_canonical.to_string_lossy().to_string()),
            kind: "worktree".to_string(),
            commit: crate::worktrunk::WtCommit {
                sha: "abc123".to_string(),
                short_sha: "abc123".to_string(),
                message: String::new(),
                timestamp: 0,
            },
            working_tree: None,
            main_state: None,
            main: None,
            remote: None,
            worktree: Some(crate::worktrunk::WtWorktreeState {
                state: None,
                detached: true,
            }),
            is_main: true,
            is_current: false,
            is_previous: false,
            symbols: None,
        };

        let converted = worktree_info_from_worktrunk(&[worktree]);

        assert_eq!(converted.len(), 1);
        assert_eq!(
            converted[0].path,
            std::fs::canonicalize(&checkout)
                .unwrap()
                .to_string_lossy()
                .to_string()
        );
        assert!(converted[0].branch.is_none());
        assert_eq!(converted[0].head_commit, "abc123");
        assert!(converted[0].is_main);
        assert!(!converted[0].is_bare);

        std::fs::remove_dir_all(root).unwrap();
    }
}
