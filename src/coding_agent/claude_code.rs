use super::CodingAgent;
use crate::types::{AgentPane, MessageSummary, PaneStatus, SessionDetail};
use jiff::Timestamp;
use serde::Deserialize;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub struct ClaudeCode;

#[derive(Debug, Deserialize)]
struct TranscriptEntry {
    #[serde(rename = "type")]
    entry_type: String,
    timestamp: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    message: Option<TranscriptMessage>,
    cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TranscriptMessage {
    role: Option<String>,
    model: Option<String>,
    usage: Option<TokenUsage>,
    content: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct TokenUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

impl CodingAgent for ClaudeCode {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn process_name(&self) -> &str {
        "claude"
    }

    fn send_prompt(
        &self,
        pane_pid: u32,
        _session_id: &str,
        prompt: &str,
    ) -> anyhow::Result<String> {
        let pane_id = find_tmux_pane_by_pid(pane_pid)
            .ok_or_else(|| anyhow::anyhow!("Could not find tmux pane for PID {}", pane_pid))?;

        let escaped = prompt.replace('\'', "'\\''");
        let status = std::process::Command::new("tmux")
            .args(["send-keys", "-t", &pane_id, &escaped, "Enter"])
            .status()
            .map_err(|e| anyhow::anyhow!("tmux send-keys failed: {}", e))?;

        if status.success() {
            Ok("Message sent to Claude Code via tmux".to_string())
        } else {
            anyhow::bail!("tmux send-keys failed with status {}", status)
        }
    }

    fn query_status(&self, pane: &AgentPane, _sys: &sysinfo::System) -> PaneStatus {
        let Some(path) = find_latest_transcript_for_path(&pane.pane_path) else {
            return PaneStatus::Unknown;
        };

        let Some(entry) = read_last_entry(&path) else {
            return PaneStatus::Unknown;
        };

        match entry.entry_type.as_str() {
            "user" | "tool_use" => PaneStatus::Busy,
            "assistant" | "tool_result" => PaneStatus::Idle,
            _ => PaneStatus::Unknown,
        }
    }

    fn enrich_pane(&self, pane: &mut AgentPane) {
        pane.agent = Some(self.name().to_string());

        let Some(path) = find_latest_transcript_for_path(&pane.pane_path) else {
            return;
        };

        let entries = read_entries(&path);
        if entries.is_empty() {
            return;
        }

        pane.db_session_id = entries.iter().rev().find_map(|e| e.session_id.clone());

        pane.db_session_title = entries
            .iter()
            .find(|e| e.entry_type == "user")
            .and_then(|e| e.message.as_ref())
            .and_then(|m| m.content.as_ref())
            .and_then(extract_text)
            .map(|s| truncate_chars(&s, 80));

        if let Some(last_assistant) = entries.iter().rev().find(|e| e.entry_type == "assistant") {
            pane.model = last_assistant
                .message
                .as_ref()
                .and_then(|m| m.model.clone());
            pane.last_activity = last_assistant
                .timestamp
                .as_deref()
                .and_then(parse_timestamp);
            pane.last_response = last_assistant
                .message
                .as_ref()
                .and_then(|m| m.content.as_ref())
                .and_then(extract_text)
                .map(|s| truncate_chars(&s, 200));
            return;
        }

        if let Some(last) = entries.last() {
            pane.model = last.message.as_ref().and_then(|m| m.model.clone());
            pane.last_activity = last.timestamp.as_deref().and_then(parse_timestamp);
        }
    }

    fn fetch_session_detail(&self, session_id: &str) -> Option<SessionDetail> {
        let path = find_transcript_for_session(session_id)?;
        let entries = read_entries(&path);
        let session_entries: Vec<&TranscriptEntry> = entries
            .iter()
            .filter(|e| e.session_id.as_deref() == Some(session_id))
            .collect();
        if session_entries.is_empty() {
            return None;
        }

        let mut input_tokens = 0_u64;
        let mut output_tokens = 0_u64;
        let mut messages: Vec<MessageSummary> = Vec::new();

        for entry in &session_entries {
            if let Some(usage) = entry.message.as_ref().and_then(|m| m.usage.as_ref()) {
                input_tokens += usage.input_tokens.unwrap_or(0);
                input_tokens += usage.cache_creation_input_tokens.unwrap_or(0);
                input_tokens += usage.cache_read_input_tokens.unwrap_or(0);
                output_tokens += usage.output_tokens.unwrap_or(0);
            }

            if let Some(timestamp) = entry.timestamp.as_deref().and_then(parse_timestamp) {
                messages.push(MessageSummary {
                    role: entry
                        .message
                        .as_ref()
                        .and_then(|m| m.role.clone())
                        .unwrap_or_else(|| entry.entry_type.clone()),
                    agent: Some(self.name().to_string()),
                    model: entry.message.as_ref().and_then(|m| m.model.clone()),
                    output_tokens: entry
                        .message
                        .as_ref()
                        .and_then(|m| m.usage.as_ref())
                        .and_then(|u| u.output_tokens)
                        .unwrap_or(0),
                    timestamp,
                    text_preview: entry
                        .message
                        .as_ref()
                        .and_then(|m| m.content.as_ref())
                        .and_then(extract_text)
                        .map(|s| truncate_chars(&s, 120)),
                });
            }
        }

        messages.sort_by_key(|m| m.timestamp);
        if messages.len() > 20 {
            let keep_from = messages.len() - 20;
            messages = messages.split_off(keep_from);
        }

        let title = session_entries
            .iter()
            .find(|e| e.entry_type == "user")
            .and_then(|e| e.message.as_ref())
            .and_then(|m| m.content.as_ref())
            .and_then(extract_text)
            .map(|s| truncate_chars(&s, 80))
            .unwrap_or_else(|| format!("Claude session {}", short_session(session_id)));

        let directory = session_entries
            .iter()
            .find_map(|e| e.cwd.clone())
            .unwrap_or_default();

        let session_created = session_entries
            .iter()
            .find_map(|e| e.timestamp.as_deref())
            .and_then(parse_timestamp);
        let session_updated = session_entries
            .iter()
            .rev()
            .find_map(|e| e.timestamp.as_deref())
            .and_then(parse_timestamp);

        Some(SessionDetail {
            session_id: session_id.to_string(),
            title,
            directory,
            message_count: session_entries.len() as u32,
            input_tokens,
            output_tokens,
            session_created,
            session_updated,
            summary_files: None,
            summary_additions: None,
            summary_deletions: None,
            messages,
            todos: Vec::new(),
        })
    }
}

fn parse_timestamp(value: &str) -> Option<Timestamp> {
    value.parse::<Timestamp>().ok()
}

fn encode_path_for_claude(path: &str) -> String {
    path.replace('/', "-")
}

fn claude_root() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude"))
}

fn find_latest_transcript_for_path(pane_path: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(pane_path.to_string());
    if let Ok(canonical) = fs::canonicalize(pane_path)
        && let Some(s) = canonical.to_str()
        && s != pane_path
    {
        candidates.push(s.to_string());
    }

    let root = claude_root()?;
    let projects_root = root.join("projects");

    for candidate in candidates {
        let encoded = encode_path_for_claude(&candidate);
        let dir = projects_root.join(encoded);
        if let Some(path) = newest_jsonl_in_dir(&dir) {
            return Some(path);
        }
    }

    newest_jsonl_in_dir(&root.join("transcripts"))
}

fn newest_jsonl_in_dir(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_jsonl(&path) {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        match &newest {
            Some((current, _)) if modified <= *current => {}
            _ => newest = Some((modified, path)),
        }
    }

    newest.map(|(_, path)| path)
}

fn is_jsonl(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
}

fn read_last_entry(path: &Path) -> Option<TranscriptEntry> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last_line: Option<String> = None;

    for line in reader.lines().map_while(Result::ok) {
        if !line.trim().is_empty() {
            last_line = Some(line);
        }
    }

    let line = last_line?;
    serde_json::from_str(&line).ok()
}

fn read_entries(path: &Path) -> Vec<TranscriptEntry> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);
    reader
        .lines()
        .map_while(Result::ok)
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<TranscriptEntry>(&line).ok())
        .collect()
}

fn find_transcript_for_session(session_id: &str) -> Option<PathBuf> {
    let files = all_transcript_files();

    let expected_name = format!("{}.jsonl", session_id);
    if let Some(path) = files.iter().find(|path| {
        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == expected_name)
    }) {
        return Some(path.clone());
    }

    files.into_iter().find(|path| {
        read_entries(path)
            .iter()
            .any(|e| e.session_id.as_deref() == Some(session_id))
    })
}

fn all_transcript_files() -> Vec<PathBuf> {
    let Some(root) = claude_root() else {
        return Vec::new();
    };

    let mut files = Vec::new();
    files.extend(collect_jsonl_files(&root.join("projects")));
    files.extend(collect_jsonl_files(&root.join("transcripts")));
    files
}

fn collect_jsonl_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(metadata) = fs::metadata(root) else {
        return files;
    };
    if !metadata.is_dir() {
        return files;
    }

    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    stack.push(path);
                } else if ft.is_file() && is_jsonl(&path) {
                    files.push(path);
                }
            }
        }
    }

    files
}

fn extract_text(content: &Value) -> Option<String> {
    match content {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                match item {
                    Value::String(s) => parts.push(s.clone()),
                    Value::Object(obj) => {
                        if let Some(text) = obj.get("text").and_then(Value::as_str) {
                            parts.push(text.to_string());
                        }
                    }
                    _ => {}
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        Value::Object(obj) => obj
            .get("text")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        _ => None,
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out: String = input.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

fn short_session(session_id: &str) -> &str {
    session_id.get(..8).unwrap_or(session_id)
}

fn find_tmux_pane_by_pid(target_pid: u32) -> Option<String> {
    let output = std::process::Command::new("tmux")
        .args(["list-panes", "-a", "-F", "#{pane_pid}\t#{pane_id}"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 && parts[0].parse::<u32>().ok() == Some(target_pid) {
            return Some(parts[1].to_string());
        }
    }

    None
}
