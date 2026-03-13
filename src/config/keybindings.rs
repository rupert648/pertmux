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
            merge_worktree: 'm',
            agent_actions: 'a',
        }
    }
}
