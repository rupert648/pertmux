---
title: Worktree Management
description: Create, remove, and merge git worktrees directly from the dashboard.
---

pertmux integrates with [worktrunk](https://github.com/max-sixty/worktrunk) (`wt`) to provide full worktree lifecycle management from within the TUI.

## Prerequisites

Install worktrunk and ensure `wt` is on your PATH:

```bash
cargo install worktrunk
```

## Worktree panel

Press **`Tab`** to switch between the MR list and the worktree panel. Each worktree card shows:

- **Branch name** with ahead/behind indicators
- **Last commit message**
- **Commit age** (e.g., "2h ago", "3d ago")
- **Git status symbols** (modified, staged, untracked files)

## Actions

| Key | Action |
|-----|--------|
| `Tab` | Switch to worktree panel |
| `c` | Create a new worktree |
| `d` | Delete selected worktree |
| `m` | Merge selected worktree into the default branch |
| `Enter` | Jump to the worktree's tmux pane |

## Create workflow

When you press `c`, pertmux opens a popup dialog where you enter the branch name. It then runs `wt create` to:

1. Create a new worktree directory
2. Create and checkout the branch
3. Refresh the dashboard to show the new worktree

## Merge workflow

Press `m` on a worktree to merge it into the default branch. pertmux runs `wt merge` which:

1. Merges the branch into the default branch
2. Cleans up the worktree directory
3. Removes the local branch
