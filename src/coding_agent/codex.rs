use super::CodingAgent;
use crate::types::{AgentPane, MessageSummary, PaneStatus, SessionDetail};
use jiff::Timestamp;
use rusqlite::Connection;
use std::path::PathBuf;

/// Default location of the Codex state database (threads, sessions).
const DEFAULT_STATE_DB: &str = ".codex/state_5.sqlite";
/// Default location of the Codex logs database (trace-level events).
const DEFAULT_LOGS_DB: &str = ".codex/logs_2.sqlite";

pub struct Codex {
    codex_home: Option<String>,
}

impl Codex {
    pub fn new(codex_home: Option<String>) -> Self {
        Self { codex_home }
    }
}

// ─── Database helpers ────────────────────────────────────────────────────────

fn open_state_db(codex_home: Option<&str>) -> Option<Connection> {
    let path = resolve_db_path(codex_home, DEFAULT_STATE_DB)?;
    open_readonly(&path)
}

fn open_logs_db(codex_home: Option<&str>) -> Option<Connection> {
    let path = resolve_db_path(codex_home, DEFAULT_LOGS_DB)?;
    open_readonly(&path)
}

fn resolve_db_path(codex_home: Option<&str>, default_suffix: &str) -> Option<PathBuf> {
    match codex_home {
        Some(home) => {
            let base = PathBuf::from(home);
            // If user pointed at the whole codex home, append just the filename
            let filename = PathBuf::from(default_suffix)
                .file_name()
                .unwrap()
                .to_owned();
            let path = base.join(filename);
            if path.exists() {
                return Some(path);
            }
            // Otherwise try the whole suffix from $HOME
            let path = dirs::home_dir().map(|h| h.join(default_suffix))?;
            if path.exists() { Some(path) } else { None }
        }
        None => {
            let path = dirs::home_dir().map(|h| h.join(default_suffix))?;
            if path.exists() { Some(path) } else { None }
        }
    }
}

fn open_readonly(path: &PathBuf) -> Option<Connection> {
    Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
}

// ─── Thread lookup ───────────────────────────────────────────────────────────

/// Find the most recently updated, non-archived thread whose `cwd` matches
/// the given pane path (comparing canonicalized forms).
fn find_thread_for_path(
    conn: &Connection,
    pane_path: &str,
) -> Option<(String, String, Option<String>, i64, i64)> {
    // Build a set of candidate paths (raw + canonicalized).
    let mut candidates = vec![pane_path.to_string()];
    if let Ok(canonical) = std::fs::canonicalize(pane_path)
        && let Some(s) = canonical.to_str()
        && s != pane_path
    {
        candidates.push(s.to_string());
    }

    let placeholders: Vec<String> = candidates.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
    let query = format!(
        "SELECT id, title, model, tokens_used, updated_at \
         FROM threads \
         WHERE archived = 0 AND cwd IN ({}) \
         ORDER BY updated_at DESC \
         LIMIT 1",
        placeholders.join(", ")
    );

    let mut stmt = conn.prepare(&query).ok()?;
    let params: Vec<&dyn rusqlite::types::ToSql> =
        candidates.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
    stmt.query_row(&*params, |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })
    .ok()
}

/// Look at the most recent log entries for a given thread to determine if the
/// agent is currently busy (processing a turn) or idle.
///
/// Heuristic: scan the last ~20 session-related log entries for the thread.
/// If the most recent operational span is a `user_input` dispatch with no
/// subsequent `interrupt` or turn completion, the agent is busy.
fn query_thread_status(logs_conn: &Connection, thread_id: &str) -> PaneStatus {
    let query = "
        SELECT feedback_log_body
        FROM logs
        WHERE thread_id = ?1
          AND (target LIKE 'codex_core::session%' OR target LIKE 'codex_core::tasks%')
          AND feedback_log_body IS NOT NULL
        ORDER BY ts DESC, id DESC
        LIMIT 20
    ";

    let Ok(mut stmt) = logs_conn.prepare(query) else {
        return PaneStatus::Unknown;
    };

    let rows: Vec<String> = stmt
        .query_map(rusqlite::params![thread_id], |row| {
            row.get::<_, String>(0)
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    if rows.is_empty() {
        return PaneStatus::Unknown;
    }

    // Walk from most recent to oldest. The first operation we recognise wins.
    for body in &rows {
        if body.contains("codex.op=\"interrupt\"") {
            return PaneStatus::Idle;
        }
        // Turn completion markers
        if body.contains("turn completed") || body.contains("turn_completed") {
            return PaneStatus::Idle;
        }
        // Active turn processing
        if body.contains("codex.op=\"user_input\"") {
            return PaneStatus::Busy;
        }
    }

    PaneStatus::Idle
}

// ─── Trait implementation ────────────────────────────────────────────────────

impl CodingAgent for Codex {
    fn name(&self) -> &str {
        "codex"
    }

    fn process_name(&self) -> &str {
        "codex"
    }

    fn query_status(&self, pane: &AgentPane) -> PaneStatus {
        let state_conn = match open_state_db(self.codex_home.as_deref()) {
            Some(c) => c,
            None => return PaneStatus::Unknown,
        };

        let Some((thread_id, ..)) = find_thread_for_path(&state_conn, &pane.pane_path) else {
            return PaneStatus::Unknown;
        };

        let Some(logs_conn) = open_logs_db(self.codex_home.as_deref()) else {
            return PaneStatus::Unknown;
        };

        query_thread_status(&logs_conn, &thread_id)
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
            Ok("Message sent to Codex via tmux".to_string())
        } else {
            anyhow::bail!("tmux send-keys failed with status {}", status)
        }
    }

    fn enrich_pane(&self, pane: &mut AgentPane) {
        pane.agent = Some(self.name().to_string());

        let Some(state_conn) = open_state_db(self.codex_home.as_deref()) else {
            return;
        };

        let Some((thread_id, title, model, _tokens_used, updated_at)) =
            find_thread_for_path(&state_conn, &pane.pane_path)
        else {
            return;
        };

        pane.db_session_id = Some(thread_id);
        pane.db_session_title = Some(truncate_str(&title, 80));
        pane.model = model;
        pane.last_activity = Timestamp::from_second(updated_at).ok();

        // Fetch the first_user_message for this thread to use as a title if
        // the stored title is the full conversation dump (Codex sometimes puts
        // the whole prompt in the title field).
        let first_msg_query =
            "SELECT first_user_message FROM threads WHERE id = ?1 AND first_user_message != ''";
        if let Ok(first_msg) = state_conn.query_row(
            first_msg_query,
            rusqlite::params![pane.db_session_id.as_deref().unwrap_or("")],
            |row| row.get::<_, String>(0),
        )
            && !first_msg.is_empty()
        {
            pane.db_session_title = Some(truncate_str(&first_msg, 80));
        }
    }

    fn fetch_session_detail(&self, session_id: &str) -> Option<SessionDetail> {
        let state_conn = open_state_db(self.codex_home.as_deref())?;
        try_fetch_detail(&state_conn, session_id, self.codex_home.as_deref())
    }
}

// ─── Session detail ──────────────────────────────────────────────────────────

fn try_fetch_detail(
    state_conn: &Connection,
    thread_id: &str,
    codex_home: Option<&str>,
) -> Option<SessionDetail> {
    let query = "
        SELECT id, title, cwd, model, model_provider, tokens_used,
               created_at, updated_at, first_user_message
        FROM threads
        WHERE id = ?1
    ";

    let mut stmt = state_conn.prepare(query).ok()?;
    let detail = stmt
        .query_row(rusqlite::params![thread_id], |row| {
            let title_raw: String = row.get(1)?;
            let first_msg: String = row.get(8)?;
            let title = if !first_msg.is_empty() {
                truncate_str(&first_msg, 80)
            } else {
                truncate_str(&title_raw, 80)
            };

            let tokens: i64 = row.get(5)?;

            Ok(SessionDetail {
                session_id: row.get::<_, String>(0)?,
                title,
                directory: row.get::<_, String>(2)?,
                message_count: 0,
                input_tokens: 0,
                output_tokens: tokens as u64,
                session_created: row
                    .get::<_, Option<i64>>(6)?
                    .and_then(|s| Timestamp::from_second(s).ok()),
                session_updated: row
                    .get::<_, Option<i64>>(7)?
                    .and_then(|s| Timestamp::from_second(s).ok()),
                summary_files: None,
                summary_additions: None,
                summary_deletions: None,
                messages: Vec::new(),
                todos: Vec::new(),
            })
        })
        .ok()?;

    // Enrich with message timeline from the logs database.
    let mut detail = detail;
    if let Some(logs_conn) = open_logs_db(codex_home) {
        detail.messages = fetch_message_timeline(&logs_conn, thread_id);
        detail.message_count = detail.messages.len() as u32;
    }

    Some(detail)
}

/// Build a simplified message timeline from the logs database.
///
/// We look for `codex_core::session::handlers` entries that contain
/// `Submission sub=Submission { ... op: UserInput` (user messages) and
/// turn-completion / assistant response markers.
fn fetch_message_timeline(logs_conn: &Connection, thread_id: &str) -> Vec<MessageSummary> {
    let query = "
        SELECT ts, feedback_log_body
        FROM logs
        WHERE thread_id = ?1
          AND target LIKE 'codex_core::session%'
          AND feedback_log_body IS NOT NULL
          AND (feedback_log_body LIKE '%op: UserInput%'
               OR feedback_log_body LIKE '%turn completed%'
               OR feedback_log_body LIKE '%codex.op=\"user_input\"%')
        ORDER BY ts ASC, id ASC
        LIMIT 40
    ";

    let Ok(mut stmt) = logs_conn.prepare(query) else {
        return Vec::new();
    };

    let rows: Vec<(i64, String)> = stmt
        .query_map(rusqlite::params![thread_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    let mut messages = Vec::new();
    for (ts, body) in rows {
        let timestamp = match Timestamp::from_second(ts) {
            Ok(t) => t,
            Err(_) => continue,
        };

        if body.contains("op: UserInput") {
            // Extract user text if possible
            let text_preview = extract_user_text(&body);
            messages.push(MessageSummary {
                role: "user".to_string(),
                agent: Some("codex".to_string()),
                model: None,
                output_tokens: 0,
                timestamp,
                text_preview,
            });
        } else if body.contains("turn completed") || body.contains("turn_completed") {
            messages.push(MessageSummary {
                role: "assistant".to_string(),
                agent: Some("codex".to_string()),
                model: None,
                output_tokens: 0,
                timestamp,
                text_preview: None,
            });
        }
    }

    // Keep the last 20
    if messages.len() > 20 {
        let keep_from = messages.len() - 20;
        messages = messages.split_off(keep_from);
    }

    messages
}

/// Try to extract the user's text from a Submission log line.
/// Format: `Submission sub=Submission { ... op: UserInput { items: [Text { text: "...", ...`
fn extract_user_text(body: &str) -> Option<String> {
    let marker = "text: \"";
    let start = body.find(marker)? + marker.len();
    let rest = &body[start..];
    // Find the closing quote, handling escaped quotes
    let mut chars = rest.chars();
    let mut text = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                text.push(next);
            }
        } else if ch == '"' {
            break;
        } else {
            text.push(ch);
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(truncate_str(&text, 120))
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn truncate_str(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out: String = input.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
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
