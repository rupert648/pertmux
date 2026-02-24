use crate::types::{OpenCodePane, OpenCodeStatus};
use std::process::Command;

pub fn list_opencode_panes() -> anyhow::Result<Vec<OpenCodePane>> {
    let format_str = "#{pane_id}\t#{session_name}\t#{window_index}\t#{pane_index}\t#{pane_title}\t#{pane_current_path}\t#{pane_pid}\t#{pane_current_command}";

    let output = Command::new("tmux")
        .args(["list-panes", "-a", "-F", format_str])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no server running") || stderr.contains("no current client") {
            return Ok(Vec::new());
        }
        anyhow::bail!("tmux list-panes failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut panes = Vec::new();

    for line in stdout.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 8 {
            continue;
        }

        if fields[7] != "opencode" {
            continue;
        }

        panes.push(OpenCodePane {
            pane_id: fields[0].to_string(),
            session_name: fields[1].to_string(),
            window_index: fields[2].parse().unwrap_or(0),
            pane_index: fields[3].parse().unwrap_or(0),
            pane_title: fields[4].to_string(),
            pane_path: fields[5].to_string(),
            pane_pid: fields[6].parse().unwrap_or(0),
            api_port: None,
            status: OpenCodeStatus::Unknown,
            db_session_title: None,
            agent: None,
            model: None,
            last_activity: None,
            db_session_id: None,
        });
    }

    Ok(panes)
}

/// Focus a pane on the OTHER tmux client (not the dashboard's).
/// If no other client exists, falls back to switching our own client.
pub fn switch_to_pane(pane_id: &str) -> anyhow::Result<()> {
    // Find our own session name
    let our_session = get_own_session().unwrap_or_default();

    // Find another client to control
    if let Some(other_tty) = find_other_client(&our_session) {
        Command::new("tmux")
            .args(["switch-client", "-c", &other_tty, "-t", pane_id])
            .output()?;
    } else {
        // Fallback: switch our own client
        Command::new("tmux")
            .args(["switch-client", "-t", pane_id])
            .output()?;
    }

    // Select the window and pane (server-side, affects the session)
    Command::new("tmux")
        .args(["select-window", "-t", pane_id])
        .output()?;
    Command::new("tmux")
        .args(["select-pane", "-t", pane_id])
        .output()?;
    Ok(())
}

/// Get the session name that the dashboard is running in.
fn get_own_session() -> Option<String> {
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#{session_name}"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Find a client TTY that is NOT attached to our session.
fn find_other_client(our_session: &str) -> Option<String> {
    let output = Command::new("tmux")
        .args(["list-clients", "-F", "#{client_tty}\t#{session_name}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 && parts[1] != our_session {
            return Some(parts[0].to_string());
        }
    }
    None
}
