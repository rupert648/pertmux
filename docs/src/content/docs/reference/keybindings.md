---
title: Keybindings
description: Complete keybinding reference for pertmux.
---

## Navigation

These keys are hardcoded and cannot be remapped.

| Key | Action |
|-----|--------|
| `j` / `k` or `↑` / `↓` | Navigate list |
| `Tab` | Toggle between MR list and worktree panel |
| `Enter` | Focus selected pane/worktree in tmux |

## Actions (configurable)

Action keys can be remapped via the `[keybindings]` section in your config file. Defaults shown below.

| Key | Action | Config key |
|-----|--------|------------|
| `r` | Refresh all data | `refresh` |
| `o` | Open MR in browser | `open_browser` |
| `b` | Copy selected branch name to clipboard | `copy_branch` |
| `f` | Fuzzy filter to switch project | `filter_projects` |
| `c` | Create new worktree | `create_worktree` |
| `w` | Create worktree and inject a prompt (requires `default_worktree_with_prompt`) | `open_worktree_with_prompt` |
| `d` | Delete selected worktree | `delete_worktree` |
| `m` | Open MR Overview popup (all your open MRs across all forges) | `mr_overview` |
| `M` | Merge selected worktree into default branch | `merge_worktree` |
| `a` | Open agent actions panel | `agent_actions` |
| `A` | Open Activity Feed popup — navigate recent events and jump to the relevant tmux pane or MR | `activity_feed` |

## Global

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit client (daemon keeps running) |
| `prefix+a` | Toggle dashboard popup (requires tmux config) |

## Configuring keybindings

Add a `[keybindings]` section to your config file to remap action keys. Only single characters are supported — no modifier keys or multi-key sequences. The `agent_actions` popup only shows when a worktree has an active opencode session.

```toml
[keybindings]
refresh = "R"
open_browser = "O"
copy_branch = "y"
filter_projects = "p"
create_worktree = "n"
open_worktree_with_prompt = "W"
delete_worktree = "x"
merge_worktree = "G"
mr_overview = "v"
agent_actions = "P"
activity_feed = "F"
```

Missing keys use their defaults. Each action must have a unique key — duplicates are rejected at startup with a clear error message.

Navigation keys (`j`/`k`/`↑`/`↓`/`Tab`/`Enter`/`Esc`/`q`) are not configurable.
