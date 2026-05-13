//! Sort column model and per-user persistence for the Projects overview pane.
//!
//! Kept separate from `client.rs` so the enum, its column order, and the
//! `~/.local/share/pertmux/last_sort` round-trip can be unit-tested in
//! isolation.

use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Columns shown in the Projects overview table, in display order.
///
/// `ORDER` is the single source of truth: header rendering and the
/// `next_col`/`prev_col` cycling both iterate it, so columns cannot drift
/// out of sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSort {
    Name,
    Mrs,
    Wt,
    Oc,
    Busy,
    Idle,
}

/// Display order of columns in the Projects table header.
pub const ORDER: [ProjectSort; 6] = [
    ProjectSort::Name,
    ProjectSort::Mrs,
    ProjectSort::Wt,
    ProjectSort::Oc,
    ProjectSort::Busy,
    ProjectSort::Idle,
];

impl ProjectSort {
    /// Stable identifier used in the on-disk `last_sort` file.
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectSort::Mrs => "mrs",
            ProjectSort::Wt => "wt",
            ProjectSort::Oc => "oc",
            ProjectSort::Busy => "busy",
            ProjectSort::Idle => "idle",
            ProjectSort::Name => "name",
        }
    }

    /// Human-readable label shown in the table header.
    pub fn label(self) -> &'static str {
        match self {
            ProjectSort::Mrs => "MRs",
            ProjectSort::Wt => "WT",
            ProjectSort::Oc => "OC",
            ProjectSort::Busy => "Busy",
            ProjectSort::Idle => "Idle",
            ProjectSort::Name => "Name",
        }
    }

    /// Default sort direction when first selecting this column.
    /// Numeric columns default to descending; Name to ascending.
    pub fn default_desc(self) -> bool {
        !matches!(self, ProjectSort::Name)
    }

    fn index(self) -> usize {
        ORDER
            .iter()
            .position(|c| *c == self)
            .expect("ORDER contains every ProjectSort variant")
    }

    /// Cycle right through the column order shown in the table header.
    pub fn next_col(self) -> Self {
        ORDER[(self.index() + 1) % ORDER.len()]
    }

    /// Cycle left through the column order shown in the table header.
    pub fn prev_col(self) -> Self {
        ORDER[(self.index() + ORDER.len() - 1) % ORDER.len()]
    }
}

impl FromStr for ProjectSort {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mrs" => Ok(ProjectSort::Mrs),
            "wt" => Ok(ProjectSort::Wt),
            "oc" => Ok(ProjectSort::Oc),
            "busy" => Ok(ProjectSort::Busy),
            "idle" => Ok(ProjectSort::Idle),
            "name" => Ok(ProjectSort::Name),
            _ => Err(()),
        }
    }
}

/// Resolve the persisted-sort file path. `None` if no user data dir is available.
fn last_sort_path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("pertmux").join("last_sort"))
}

/// Persist the user's chosen sort column + direction.
/// Errors are intentionally swallowed: persistence is best-effort.
pub fn save_last_sort(col: ProjectSort, desc: bool) {
    if let Some(path) = last_sort_path() {
        save_last_sort_to(&path, col, desc);
    }
}

/// Load the user's previously-chosen sort column + direction.
pub fn load_last_sort() -> Option<(ProjectSort, bool)> {
    load_last_sort_from(&last_sort_path()?)
}

// --- Path-injecting helpers (tested) -----------------------------------------

fn save_last_sort_to(path: &Path, col: ProjectSort, desc: bool) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let dir = if desc { "desc" } else { "asc" };
    let _ = std::fs::write(path, format!("{},{}", col.as_str(), dir));
}

fn load_last_sort_from(path: &Path) -> Option<(ProjectSort, bool)> {
    let raw = std::fs::read_to_string(path).ok()?;
    let s = raw.trim();
    if let Some((col_str, dir_str)) = s.split_once(',') {
        let col = ProjectSort::from_str(col_str.trim()).ok()?;
        let desc = match dir_str.trim() {
            "desc" => true,
            "asc" => false,
            _ => return None,
        };
        Some((col, desc))
    } else {
        // Legacy single-token form: just the column name, no direction.
        let col = ProjectSort::from_str(s).ok()?;
        Some((col, col.default_desc()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_from_str_round_trip() {
        for col in ORDER {
            let parsed: ProjectSort = col.as_str().parse().unwrap();
            assert_eq!(parsed, col, "round-trip failed for {:?}", col);
        }
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert!("bogus".parse::<ProjectSort>().is_err());
        assert!("".parse::<ProjectSort>().is_err());
    }

    #[test]
    fn next_prev_are_inverse() {
        for col in ORDER {
            assert_eq!(col.next_col().prev_col(), col);
            assert_eq!(col.prev_col().next_col(), col);
        }
    }

    #[test]
    fn next_col_cycles_full_circle() {
        let mut c = ProjectSort::Name;
        for _ in 0..ORDER.len() {
            c = c.next_col();
        }
        assert_eq!(c, ProjectSort::Name);
    }

    #[test]
    fn default_desc_matches_intent() {
        assert!(!ProjectSort::Name.default_desc());
        for col in [
            ProjectSort::Mrs,
            ProjectSort::Wt,
            ProjectSort::Oc,
            ProjectSort::Busy,
            ProjectSort::Idle,
        ] {
            assert!(col.default_desc());
        }
    }

    #[test]
    fn save_load_round_trip_desc() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("last_sort");
        save_last_sort_to(&path, ProjectSort::Busy, true);
        assert_eq!(load_last_sort_from(&path), Some((ProjectSort::Busy, true)));
    }

    #[test]
    fn save_load_round_trip_asc() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("last_sort");
        save_last_sort_to(&path, ProjectSort::Name, false);
        assert_eq!(load_last_sort_from(&path), Some((ProjectSort::Name, false)));
    }

    #[test]
    fn load_legacy_single_token_uses_default_direction() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("last_sort");
        std::fs::write(&path, "mrs").unwrap();
        // Numeric column → desc by default.
        assert_eq!(load_last_sort_from(&path), Some((ProjectSort::Mrs, true)));

        std::fs::write(&path, "name").unwrap();
        // Name → asc by default.
        assert_eq!(load_last_sort_from(&path), Some((ProjectSort::Name, false)));
    }

    #[test]
    fn load_rejects_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("last_sort");
        std::fs::write(&path, "mrs,sideways").unwrap();
        assert!(load_last_sort_from(&path).is_none());

        std::fs::write(&path, "garbage,desc").unwrap();
        assert!(load_last_sort_from(&path).is_none());
    }

    #[test]
    fn load_missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope");
        assert!(load_last_sort_from(&path).is_none());
    }
}
