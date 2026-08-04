use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SESSION_FILE: &str = "pertmux/codex-session-id";
const CODEX_SUBCOMMANDS: &[&str] = &[
    "exec",
    "e",
    "review",
    "login",
    "logout",
    "mcp",
    "plugin",
    "mcp-server",
    "app-server",
    "remote-control",
    "app",
    "completion",
    "update",
    "doctor",
    "sandbox",
    "debug",
    "apply",
    "a",
    "resume",
    "fork",
    "cloud",
    "exec-server",
    "features",
    "help",
];

pub fn persist(worktree_path: &str, session_id: &str) -> Result<()> {
    let session_id = session_id.trim();
    if !valid_session_id(session_id) {
        anyhow::bail!("invalid Codex session ID");
    }

    let path = session_file_path(worktree_path)?;
    if fs::read_to_string(&path)
        .ok()
        .is_some_and(|stored| stored.trim() == session_id)
    {
        return Ok(());
    }

    let parent = path
        .parent()
        .context("Codex session metadata path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::write(&path, format!("{session_id}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

pub fn read(worktree_path: &str) -> Option<String> {
    let path = session_file_path(worktree_path).ok()?;
    let session_id = fs::read_to_string(path).ok()?;
    let session_id = session_id.trim();
    valid_session_id(session_id).then(|| session_id.to_string())
}

pub fn resume_command(worktree_path: &str, command: &str) -> String {
    let Some(session_id) = read(worktree_path) else {
        return command.to_string();
    };
    insert_resume(command, &session_id).unwrap_or_else(|| command.to_string())
}

fn session_file_path(worktree_path: &str) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["-C", worktree_path, "rev-parse", "--git-path", SESSION_FILE])
        .output()
        .with_context(|| format!("failed to inspect Git metadata for {worktree_path}"))?;

    if !output.status.success() {
        anyhow::bail!("{} is not inside a Git worktree", worktree_path);
    }

    let raw = String::from_utf8(output.stdout).context("Git returned a non-UTF-8 metadata path")?;
    let path = PathBuf::from(raw.trim());
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(Path::new(worktree_path).join(path))
    }
}

fn insert_resume(command: &str, session_id: &str) -> Option<String> {
    let trimmed = command.trim_start();
    let executable_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let executable = &trimmed[..executable_end];
    let executable_name = Path::new(executable).file_name()?.to_str()?;
    if executable_name != "codex" {
        return None;
    }

    let remainder = &trimmed[executable_end..];
    let first_arg = remainder.split_whitespace().next();
    if first_arg.is_some_and(|arg| CODEX_SUBCOMMANDS.contains(&arg)) {
        return None;
    }

    let leading = &command[..command.len() - trimmed.len()];
    Some(format!(
        "{leading}{executable} resume {session_id}{remainder}"
    ))
}

fn valid_session_id(session_id: &str) -> bool {
    !session_id.is_empty()
        && session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn inserts_resume_for_interactive_codex() {
        assert_eq!(
            insert_resume("codex", "abc-123"),
            Some("codex resume abc-123".to_string())
        );
        assert_eq!(
            insert_resume("codex implement the feature", "abc-123"),
            Some("codex resume abc-123 implement the feature".to_string())
        );
        assert_eq!(
            insert_resume("/opt/bin/codex --search", "abc-123"),
            Some("/opt/bin/codex resume abc-123 --search".to_string())
        );
    }

    #[test]
    fn leaves_noninteractive_or_existing_resume_commands_alone() {
        assert_eq!(insert_resume("codex exec task", "abc-123"), None);
        assert_eq!(insert_resume("codex resume other-id", "abc-123"), None);
        assert_eq!(insert_resume("opencode", "abc-123"), None);
    }

    #[test]
    fn validates_session_ids_for_shell_safety() {
        assert!(valid_session_id("019c2d73-4f67-7d10-8b75-071bc8433f0a"));
        assert!(!valid_session_id(""));
        assert!(!valid_session_id("id; rm -rf /"));
    }

    #[test]
    fn persists_session_in_git_metadata_from_nested_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let repo = std::env::temp_dir().join(format!("pertmux-codex-session-{unique}"));
        let nested = repo.join("src");
        fs::create_dir_all(&nested).unwrap();
        let status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo)
            .status()
            .unwrap();
        assert!(status.success());

        persist(nested.to_str().unwrap(), "abc-123").unwrap();

        assert_eq!(read(repo.to_str().unwrap()).as_deref(), Some("abc-123"));
        assert!(!repo.join("pertmux/codex-session-id").exists());
        fs::remove_dir_all(repo).unwrap();
    }
}
