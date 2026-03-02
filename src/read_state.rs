use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

pub struct ReadStateDb {
    conn: Connection,
}

impl ReadStateDb {
    /// Open (or create) the read state database.
    /// If `path` is None, uses `~/.local/share/pertmux/read_state.db`.
    pub fn open(path: Option<&str>) -> Result<Self> {
        let db_path = match path {
            Some(p) => std::path::PathBuf::from(p),
            None => {
                let data_dir = dirs::data_dir().context("Could not determine data directory")?;
                let pertmux_dir = data_dir.join("pertmux");
                std::fs::create_dir_all(&pertmux_dir)
                    .context("Failed to create pertmux data directory")?;
                pertmux_dir.join("read_state.db")
            }
        };

        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .context(format!("Failed to open read_state.db at {:?}", db_path))?;

        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS seen_notes (
                    note_id   INTEGER NOT NULL,
                    mr_iid    INTEGER NOT NULL,
                    project   TEXT    NOT NULL,
                    seen_at   TEXT    NOT NULL DEFAULT (datetime('now')),
                    PRIMARY KEY (note_id, mr_iid, project)
                );
                CREATE TABLE IF NOT EXISTS mr_last_viewed (
                    mr_iid          INTEGER NOT NULL,
                    project         TEXT    NOT NULL,
                    last_viewed_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                    last_note_count INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mr_iid, project)
                );",
            )
            .context("Failed to run database migration")
    }

    /// Mark a set of note IDs as seen for a given MR.
    pub fn mark_notes_seen(&self, project: &str, mr_iid: u64, note_ids: &[u64]) -> Result<()> {
        for &note_id in note_ids {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO seen_notes (note_id, mr_iid, project) VALUES (?1, ?2, ?3)",
                    params![note_id as i64, mr_iid as i64, project],
                )
                .context("Failed to mark note as seen")?;
        }
        Ok(())
    }

    /// Returns count of note IDs from `all_note_ids` that have NOT been seen yet.
    pub fn get_unseen_note_count(
        &self,
        project: &str,
        mr_iid: u64,
        all_note_ids: &[u64],
    ) -> Result<usize> {
        if all_note_ids.is_empty() {
            return Ok(0);
        }

        let mut unseen_count = 0;
        for &note_id in all_note_ids {
            let seen: bool = self
                .conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM seen_notes WHERE note_id = ?1 AND mr_iid = ?2 AND project = ?3",
                    params![note_id as i64, mr_iid as i64, project],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if !seen {
                unseen_count += 1;
            }
        }
        Ok(unseen_count)
    }

    /// Record that an MR was viewed and what the note count was at that time.
    pub fn mark_mr_viewed(&self, project: &str, mr_iid: u64, note_count: u32) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO mr_last_viewed (mr_iid, project, last_viewed_at, last_note_count)
                 VALUES (?1, ?2, datetime('now'), ?3)
                 ON CONFLICT(mr_iid, project) DO UPDATE SET
                    last_viewed_at = datetime('now'),
                    last_note_count = ?3",
                params![mr_iid as i64, project, note_count],
            )
            .context("Failed to update mr_last_viewed")?;
        Ok(())
    }

    /// Returns true if the current note count exceeds the last-seen count.
    /// Returns false if no prior view record exists (conservative — no false positives on first open).
    pub fn has_new_activity(
        &self,
        project: &str,
        mr_iid: u64,
        current_note_count: u32,
    ) -> Result<bool> {
        let last_count: Option<i64> = self
            .conn
            .query_row(
                "SELECT last_note_count FROM mr_last_viewed WHERE mr_iid = ?1 AND project = ?2",
                params![mr_iid as i64, project],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to query mr_last_viewed")?;

        Ok(match last_count {
            None => false,
            Some(last) => current_note_count as i64 > last,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_db() -> ReadStateDb {
        let conn = Connection::open_in_memory().expect("Failed to open in-memory DB");
        let db = ReadStateDb { conn };
        db.migrate().expect("Migration failed");
        db
    }

    #[test]
    fn test_mark_and_query_unseen_notes() {
        let db = open_test_db();
        let unseen = db
            .get_unseen_note_count("project", 1, &[1, 2, 3, 4, 5])
            .unwrap();
        assert_eq!(unseen, 5);

        db.mark_notes_seen("project", 1, &[1, 2, 3]).unwrap();
        let unseen = db
            .get_unseen_note_count("project", 1, &[1, 2, 3, 4, 5])
            .unwrap();
        assert_eq!(unseen, 2);

        db.mark_notes_seen("project", 1, &[4, 5]).unwrap();
        let unseen = db
            .get_unseen_note_count("project", 1, &[1, 2, 3, 4, 5])
            .unwrap();
        assert_eq!(unseen, 0);
    }

    #[test]
    fn test_mark_seen_idempotent() {
        let db = open_test_db();
        db.mark_notes_seen("project", 1, &[10, 20]).unwrap();
        db.mark_notes_seen("project", 1, &[10, 20]).unwrap();
        let unseen = db.get_unseen_note_count("project", 1, &[10, 20]).unwrap();
        assert_eq!(unseen, 0);
    }

    #[test]
    fn test_has_new_activity_no_prior_record() {
        let db = open_test_db();
        assert!(!db.has_new_activity("project", 42, 5).unwrap());
    }

    #[test]
    fn test_has_new_activity_count_increased() {
        let db = open_test_db();
        db.mark_mr_viewed("project", 42, 5).unwrap();
        assert!(!db.has_new_activity("project", 42, 5).unwrap());
        assert!(db.has_new_activity("project", 42, 8).unwrap());
    }

    #[test]
    fn test_multiple_mrs_tracked_independently() {
        let db = open_test_db();
        db.mark_mr_viewed("project", 1, 3).unwrap();
        db.mark_mr_viewed("project", 2, 7).unwrap();
        assert!(!db.has_new_activity("project", 1, 3).unwrap());
        assert!(db.has_new_activity("project", 2, 10).unwrap());
        assert!(!db.has_new_activity("project", 1, 3).unwrap());
    }

    #[test]
    fn test_empty_note_ids() {
        let db = open_test_db();
        let unseen = db.get_unseen_note_count("project", 1, &[]).unwrap();
        assert_eq!(unseen, 0);
    }
}
