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
| `d` | Delete selected worktree | `delete_worktree` |
| `m` | Merge selected worktree into default branch | `merge_worktree` |

## Global

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit client (daemon keeps running) |
| `prefix+a` | Toggle dashboard popup (requires tmux config) |

## Configuring keybindings

Add a `[keybindings]` section to your config file to remap action keys. Only single characters are supported — no modifier keys or multi-key sequences.

```toml
[keybindings]
refresh = "R"
open_browser = "O"
copy_branch = "y"
filter_projects = "p"
create_worktree = "n"
delete_worktree = "x"
merge_worktree = "g"
```

Missing keys use their defaults. Each action must have a unique key — duplicates are rejected at startup with a clear error message.

Navigation keys (`j`/`k`/`↑`/`↓`/`Tab`/`Enter`/`Esc`/`q`) are not configurable.
