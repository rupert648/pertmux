use crate::types::{AgentPane, MessageSummary, SessionDetail, TodoItem};
use jiff::Timestamp;
use rusqlite::Connection;

const DEFAULT_DB_PATH_SUFFIX: &str = ".local/share/opencode/opencode.db";

fn open_db(db_path_override: Option<&str>) -> Option<Connection> {
    let db_path = match db_path_override {
        Some(p) => std::path::PathBuf::from(p),
        None => dirs::home_dir().map(|h| h.join(DEFAULT_DB_PATH_SUFFIX))?,
    };
    if !db_path.exists() {
        return None;
    }
    Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
}

pub fn enrich_pane(pane: &mut AgentPane, db_path: Option<&str>) {
    let _ = try_enrich(pane, db_path);
}

fn try_enrich(pane: &mut AgentPane, db_path: Option<&str>) -> anyhow::Result<()> {
    let conn = open_db(db_path).ok_or_else(|| anyhow::anyhow!("no db"))?;

    let title_raw = pane
        .pane_title
        .strip_prefix("OC | ")
        .unwrap_or(&pane.pane_title);
    // Strip trailing "..." from tmux-truncated titles
    let title_prefix = title_raw.strip_suffix("...").unwrap_or(title_raw);

    // Escape % and _ for LIKE
    let escaped = title_prefix.replace('%', "\\%").replace('_', "\\_");
    let like_pattern = format!("{}%", escaped);

    let query = "
        SELECT s.id,
               s.title,
               json_extract(m.data, '$.agent') as agent,
               json_extract(m.data, '$.modelID') as model,
               s.time_updated,
               (SELECT substr(json_extract(p.data, '$.text'), 1, 200)
                FROM part p
                JOIN message mlast ON p.message_id = mlast.id
                WHERE mlast.session_id = s.id
                  AND json_extract(mlast.data, '$.role') = 'assistant'
                  AND json_extract(p.data, '$.type') = 'text'
                ORDER BY mlast.time_created DESC
                LIMIT 1) as last_response
        FROM session s
        LEFT JOIN message m ON m.session_id = s.id
            AND m.time_created = (
                SELECT MAX(m2.time_created) FROM message m2 WHERE m2.session_id = s.id
            )
        WHERE s.title LIKE ?1 ESCAPE '\\'
          AND s.directory = ?2
          AND s.time_archived IS NULL
        ORDER BY s.time_updated DESC
        LIMIT 1
    ";

    let mut stmt = conn.prepare(query)?;
    let result = stmt.query_row(rusqlite::params![like_pattern, pane.pane_path], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<i64>>(4)?,
            row.get::<_, Option<String>>(5)?,
        ))
    });

    if let Ok((session_id, title, agent, model, updated, last_response)) = result {
        pane.db_session_id = session_id;
        pane.db_session_title = title;
        pane.agent = agent;
        pane.model = model;
        pane.last_activity = updated.and_then(|ms| Timestamp::from_millisecond(ms).ok());
        pane.last_response = last_response;
    }
    Ok(())
}

/// Fetch detailed session info for the detail panel.
pub fn fetch_session_detail(session_id: &str, db_path: Option<&str>) -> Option<SessionDetail> {
    let conn = open_db(db_path)?;
    try_fetch_detail(&conn, session_id).ok()
}

fn try_fetch_detail(conn: &Connection, session_id: &str) -> anyhow::Result<SessionDetail> {
    // Session stats
    let stats_query = "
        SELECT s.id, s.title, s.directory,
               COUNT(DISTINCT m.id) as msg_count,
               COALESCE(SUM(json_extract(m.data, '$.tokens.input')), 0) as total_input,
               COALESCE(SUM(json_extract(m.data, '$.tokens.output')), 0) as total_output,
               s.time_created, s.time_updated,
               s.summary_files, s.summary_additions, s.summary_deletions
        FROM session s
        LEFT JOIN message m ON m.session_id = s.id
        WHERE s.id = ?1
        GROUP BY s.id
    ";

    let mut detail = conn.query_row(stats_query, rusqlite::params![session_id], |row| {
        Ok(SessionDetail {
            session_id: row.get::<_, String>(0)?,
            title: row.get::<_, String>(1)?,
            directory: row.get::<_, String>(2)?,
            message_count: row.get::<_, u32>(3)?,
            input_tokens: row.get::<_, u64>(4)?,
            output_tokens: row.get::<_, u64>(5)?,
            session_created: row
                .get::<_, Option<i64>>(6)?
                .and_then(|ms| Timestamp::from_millisecond(ms).ok()),
            session_updated: row
                .get::<_, Option<i64>>(7)?
                .and_then(|ms| Timestamp::from_millisecond(ms).ok()),
            summary_files: row.get::<_, Option<u32>>(8)?,
            summary_additions: row.get::<_, Option<u32>>(9)?,
            summary_deletions: row.get::<_, Option<u32>>(10)?,
            messages: Vec::new(),
            todos: Vec::new(),
        })
    })?;

    // Recent messages (last 20 turns with text preview)
    let msg_query = "
        SELECT json_extract(m.data, '$.role') as role,
               json_extract(m.data, '$.agent') as agent,
               json_extract(m.data, '$.modelID') as model,
               COALESCE(json_extract(m.data, '$.tokens.output'), 0) as out_tokens,
               m.time_created,
               (SELECT substr(json_extract(p.data, '$.text'), 1, 120)
                FROM part p
                WHERE p.message_id = m.id
                  AND json_extract(p.data, '$.type') = 'text'
                LIMIT 1) as text_preview
        FROM message m
        WHERE m.session_id = ?1
        ORDER BY m.time_created DESC
        LIMIT 20
    ";

    let mut stmt = conn.prepare(msg_query)?;
    let messages: Vec<MessageSummary> = stmt
        .query_map(rusqlite::params![session_id], |row| {
            let ts_ms = row.get::<_, i64>(4)?;
            let timestamp = Timestamp::from_millisecond(ts_ms)
                .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(4, ts_ms))?;

            Ok(MessageSummary {
                role: row.get::<_, String>(0)?,
                agent: row.get::<_, Option<String>>(1)?,
                model: row.get::<_, Option<String>>(2)?,
                output_tokens: row.get::<_, u64>(3)?,
                timestamp,
                text_preview: row.get::<_, Option<String>>(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Reverse so oldest first (timeline order)
    detail.messages = messages.into_iter().rev().collect();

    // Todos
    let todo_query = "
        SELECT content, status, priority
        FROM todo
        WHERE session_id = ?1
        ORDER BY position ASC
    ";

    let mut stmt = conn.prepare(todo_query)?;
    detail.todos = stmt
        .query_map(rusqlite::params![session_id], |row| {
            Ok(TodoItem {
                content: row.get::<_, String>(0)?,
                status: row.get::<_, String>(1)?,
                priority: row.get::<_, String>(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(detail)
}
