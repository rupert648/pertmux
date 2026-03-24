use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub refresh: char,
    pub open_browser: char,
    pub copy_branch: char,
    pub filter_projects: char,
    pub create_worktree: char,
    pub delete_worktree: char,
    pub merge_worktree: char,
    pub agent_actions: char,
    pub mr_overview: char,
    pub activity_feed: char,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            refresh: 'r',
            open_browser: 'o',
            copy_branch: 'b',
            filter_projects: 'f',
            create_worktree: 'c',
            delete_worktree: 'd',
            merge_worktree: 'M',
            agent_actions: 'a',
            mr_overview: 'm',
            activity_feed: 'A',
        }
    }
}

impl KeybindingsConfig {
    /// Returns all configurable keybindings as (key, description) pairs in
    /// display order. The keybindings help modal iterates this list, so adding
    /// a new entry here automatically surfaces it in the modal.
    pub fn entries(&self) -> Vec<(char, &'static str)> {
        vec![
            (self.refresh, "Refresh data"),
            (self.open_browser, "Open MR in browser"),
            (self.copy_branch, "Copy branch name"),
            (self.filter_projects, "Switch project"),
            (self.mr_overview, "My open MRs"),
            (self.activity_feed, "Activity feed"),
            (self.agent_actions, "Agent actions"),
            (self.create_worktree, "Create worktree"),
            (self.delete_worktree, "Delete worktree"),
            (self.merge_worktree, "Merge worktree"),
        ]
    }
}
