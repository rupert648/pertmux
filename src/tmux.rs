use crate::types::{AgentPane, PaneStatus};
use std::path::Path;
use std::process::Command;

pub fn list_agent_panes(process_names: &[&str]) -> anyhow::Result<Vec<AgentPane>> {
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

        if !process_names.iter().any(|name| *name == fields[7]) {
            continue;
        }

        panes.push(AgentPane {
            pane_id: fields[0].to_string(),
            session_name: fields[1].to_string(),
            window_index: fields[2].parse().unwrap_or(0),
            pane_index: fields[3].parse().unwrap_or(0),
            pane_title: fields[4].to_string(),
            pane_path: fields[5].to_string(),
            pane_pid: fields[6].parse().unwrap_or(0),
            pane_command: fields[7].to_string(),
            status: PaneStatus::Unknown,
            db_session_title: None,
            agent: None,
            model: None,
            last_activity: None,
            db_session_id: None,
            last_response: None,
        });
    }

    Ok(panes)
}

pub fn find_or_create_pane(
    path: &str,
    project_name: &str,
    agent_command: Option<&str>,
) -> anyhow::Result<()> {
    let canonical_target =
        std::fs::canonicalize(path).unwrap_or_else(|_| Path::new(path).to_path_buf());

    if let Some(pane_id) = find_pane_by_path(&canonical_target)? {
        return switch_to_pane(&pane_id);
    }

    let our_session = get_own_session().unwrap_or_default();

    let target_session = find_session_by_name(project_name).unwrap_or_else(|| {
        if let Some(other_tty) = find_other_client(&our_session) {
            session_for_client(&other_tty).unwrap_or(our_session.clone())
        } else {
            our_session.clone()
        }
    });

    let window_name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| project_name.to_string());

    let output = Command::new("tmux")
        .args([
            "new-window",
            "-a",
            "-t",
            &target_session,
            "-n",
            &window_name,
            "-c",
            path,
            "-P",
            "-F",
            "#{pane_id}",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tmux new-window failed: {}", stderr.trim());
    }

    let new_pane_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !new_pane_id.is_empty() {
        if let Some(cmd) = agent_command {
            let split_output = Command::new("tmux")
                .args([
                    "split-window",
                    "-h",
                    "-t",
                    &new_pane_id,
                    "-c",
                    path,
                    "-P",
                    "-F",
                    "#{pane_id}",
                ])
                .output()?;

            if !split_output.status.success() {
                let stderr = String::from_utf8_lossy(&split_output.stderr);
                anyhow::bail!("tmux split-window failed: {}", stderr.trim());
            }

            Command::new("tmux")
                .args(["send-keys", "-t", &new_pane_id, cmd, "Enter"])
                .output()?;

            switch_to_pane(&new_pane_id)?;
        } else {
            switch_to_pane(&new_pane_id)?;
        }
    }

    Ok(())
}

fn find_pane_by_path(target: &Path) -> anyhow::Result<Option<String>> {
    let output = Command::new("tmux")
        .args(["list-panes", "-a", "-F", "#{pane_id}\t#{pane_current_path}"])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let pane_id = parts[0];
        let pane_path =
            std::fs::canonicalize(parts[1]).unwrap_or_else(|_| Path::new(parts[1]).to_path_buf());

        if pane_path == target {
            return Ok(Some(pane_id.to_string()));
        }
    }

    Ok(None)
}

pub fn switch_to_pane(pane_id: &str) -> anyhow::Result<()> {
    let our_session = get_own_session().unwrap_or_default();

    if let Some(other_tty) = find_other_client(&our_session) {
        Command::new("tmux")
            .args(["switch-client", "-c", &other_tty, "-t", pane_id])
            .output()?;
    } else {
        Command::new("tmux")
            .args(["switch-client", "-t", pane_id])
            .output()?;
    }

    Command::new("tmux")
        .args(["select-window", "-t", pane_id])
        .output()?;
    Command::new("tmux")
        .args(["select-pane", "-t", pane_id])
        .output()?;
    Ok(())
}

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

fn session_for_client(client_tty: &str) -> Option<String> {
    let output = Command::new("tmux")
        .args(["display-message", "-t", client_tty, "-p", "#{session_name}"])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn find_session_by_name(name: &str) -> Option<String> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lower_name = name.to_lowercase();
    stdout
        .lines()
        .find(|s| s.to_lowercase() == lower_name)
        .map(|s| s.to_string())
}

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
