//! Aggregated, sortable per-project stats for the Projects overview pane.
//!
//! Shared by the client (for cursor/selection mapping) and the overview
//! renderer (for the visible table). Building this once per snapshot avoids
//! double-walking `snapshot.panes` on every render.

use crate::project_sort::ProjectSort;
use crate::protocol::{DashboardSnapshot, ProjectSnapshot};
use crate::types::{AgentPane, PaneStatus};

/// One row of the Projects overview table.
#[derive(Debug, Clone)]
pub struct ProjectStats {
    /// Index into `snapshot.projects` — the canonical project identity.
    /// Stays stable regardless of sort order.
    pub canonical_idx: usize,
    pub name: String,
    pub mrs: usize,
    pub wt: usize,
    pub busy: usize,
    pub idle: usize,
}

impl ProjectStats {
    /// Total agent presence (Busy + Idle).
    pub fn oc(&self) -> usize {
        self.busy + self.idle
    }

    /// Value used for sort comparisons.
    fn key_for(&self, col: ProjectSort) -> SortKey<'_> {
        match col {
            ProjectSort::Name => SortKey::Str(&self.name),
            ProjectSort::Mrs => SortKey::Num(self.mrs),
            ProjectSort::Wt => SortKey::Num(self.wt),
            ProjectSort::Oc => SortKey::Num(self.oc()),
            ProjectSort::Busy => SortKey::Num(self.busy),
            ProjectSort::Idle => SortKey::Num(self.idle),
        }
    }
}

enum SortKey<'a> {
    Str(&'a str),
    Num(usize),
}

impl<'a> SortKey<'a> {
    fn cmp(&self, other: &SortKey<'a>) -> std::cmp::Ordering {
        match (self, other) {
            (SortKey::Str(a), SortKey::Str(b)) => a.cmp(b),
            (SortKey::Num(a), SortKey::Num(b)) => a.cmp(b),
            // Mixed variants never occur — same column for both sides.
            _ => std::cmp::Ordering::Equal,
        }
    }
}

/// Build the full set of project stats from the current snapshot.
///
/// Single-pass over `snapshot.panes` per project, classifying each matching
/// pane as Busy or Idle in one fold. Path comparison is done with the
/// pre-canonicalized `pane.canonical_path` when available, falling back to
/// trimmed `pane_path`.
pub fn build_project_stats(snapshot: &DashboardSnapshot) -> Vec<ProjectStats> {
    snapshot
        .projects
        .iter()
        .enumerate()
        .map(|(i, proj)| {
            let project_paths = project_paths(proj);
            let (busy, idle) = snapshot.panes.iter().fold((0, 0), |(b, i), pane| {
                if !pane_belongs(pane, &project_paths) {
                    return (b, i);
                }
                match pane.status {
                    PaneStatus::Busy => (b + 1, i),
                    PaneStatus::Idle => (b, i + 1),
                    _ => (b, i),
                }
            });
            ProjectStats {
                canonical_idx: i,
                name: proj.name.clone(),
                mrs: proj.dashboard.linked_mrs.len(),
                wt: proj.cached_worktrees.len().saturating_sub(1),
                busy,
                idle,
            }
        })
        .collect()
}

/// Sort `stats` in place by `col`, respecting `desc`. Ties broken by name
/// ascending **regardless of direction** so users see a stable secondary order.
pub fn sort_stats(stats: &mut [ProjectStats], col: ProjectSort, desc: bool) {
    stats.sort_by(|a, b| {
        let primary = a.key_for(col).cmp(&b.key_for(col));
        let primary = if desc { primary.reverse() } else { primary };
        // Hold the secondary in its own binding so the desc flip above
        // cannot accidentally reverse the tie-break too.
        primary.then_with(|| a.name.cmp(&b.name))
    });
}

/// Convenience: build + sort in one call.
pub fn build_sorted_project_stats(
    snapshot: &DashboardSnapshot,
    col: ProjectSort,
    desc: bool,
) -> Vec<ProjectStats> {
    let mut stats = build_project_stats(snapshot);
    sort_stats(&mut stats, col, desc);
    stats
}

// --- internals ---------------------------------------------------------------

/// All filesystem paths that belong to a project (its checkout + worktree dirs),
/// with trailing slashes pre-stripped for cheap comparison.
fn project_paths(proj: &ProjectSnapshot) -> Vec<&str> {
    std::iter::once(proj.local_path.as_str())
        .chain(
            proj.cached_worktrees
                .iter()
                .filter_map(|wt| wt.path.as_deref()),
        )
        .map(|p| p.trim_end_matches('/'))
        .collect()
}

fn pane_belongs(pane: &AgentPane, project_paths: &[&str]) -> bool {
    let pane_path = pane
        .canonical_path
        .as_deref()
        .unwrap_or(pane.pane_path.as_str())
        .trim_end_matches('/');
    project_paths.contains(&pane_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(name: &str, mrs: usize, busy: usize, idle: usize, idx: usize) -> ProjectStats {
        ProjectStats {
            canonical_idx: idx,
            name: name.to_string(),
            mrs,
            wt: 0,
            busy,
            idle,
        }
    }

    #[test]
    fn sort_desc_ties_broken_by_name_ascending() {
        let mut v = vec![
            stats("charlie", 3, 0, 0, 0),
            stats("alpha", 3, 0, 0, 1),
            stats("bravo", 3, 0, 0, 2),
        ];
        sort_stats(&mut v, ProjectSort::Mrs, true);
        // All have mrs=3, so tie-break must yield alpha, bravo, charlie even
        // though primary direction is descending.
        let names: Vec<&str> = v.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn sort_asc_orders_smallest_first() {
        let mut v = vec![
            stats("a", 5, 0, 0, 0),
            stats("b", 1, 0, 0, 1),
            stats("c", 3, 0, 0, 2),
        ];
        sort_stats(&mut v, ProjectSort::Mrs, false);
        assert_eq!(v.iter().map(|s| s.mrs).collect::<Vec<_>>(), vec![1, 3, 5]);
    }

    #[test]
    fn sort_desc_orders_largest_first() {
        let mut v = vec![
            stats("a", 5, 0, 0, 0),
            stats("b", 1, 0, 0, 1),
            stats("c", 3, 0, 0, 2),
        ];
        sort_stats(&mut v, ProjectSort::Mrs, true);
        assert_eq!(v.iter().map(|s| s.mrs).collect::<Vec<_>>(), vec![5, 3, 1]);
    }

    #[test]
    fn sort_by_name_ascending() {
        let mut v = vec![
            stats("charlie", 0, 0, 0, 0),
            stats("alpha", 0, 0, 0, 1),
            stats("bravo", 0, 0, 0, 2),
        ];
        sort_stats(&mut v, ProjectSort::Name, false);
        assert_eq!(
            v.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "bravo", "charlie"]
        );
    }

    #[test]
    fn oc_is_busy_plus_idle() {
        let s = stats("p", 0, 2, 5, 0);
        assert_eq!(s.oc(), 7);
    }

    #[test]
    fn sort_by_oc_uses_sum() {
        let mut v = vec![
            stats("a", 0, 1, 1, 0), // oc=2
            stats("b", 0, 5, 0, 1), // oc=5
            stats("c", 0, 0, 3, 2), // oc=3
        ];
        sort_stats(&mut v, ProjectSort::Oc, true);
        assert_eq!(
            v.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["b", "c", "a"]
        );
    }
}
