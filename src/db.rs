use crate::types::OpenCodePane;
use rusqlite::Connection;

const DB_PATH_SUFFIX: &str = ".local/share/opencode/opencode.db";

fn get_db_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(DB_PATH_SUFFIX))
}

pub fn enrich_pane(pane: &mut OpenCodePane) {
    if let Err(_e) = try_enrich(pane) {
        // DB errors are non-fatal; pane keeps default values
    }
}

fn try_enrich(pane: &mut OpenCodePane) -> anyhow::Result<()> {
    let db_path = get_db_path().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
    if !db_path.exists() {
        return Ok(());
    }
    let conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

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
        SELECT s.title,
               json_extract(m.data, '$.agent') as agent,
               json_extract(m.data, '$.modelID') as model,
               s.time_updated
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
            row.get::<_, Option<i64>>(3)?,
        ))
    });

    if let Ok((title, agent, model, updated)) = result {
        pane.db_session_title = title;
        pane.agent = agent;
        pane.model = model;
        pane.last_activity = updated;
    }
    Ok(())
}
