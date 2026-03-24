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

The worktree panel is rendered at the **top** of the dashboard, with the MR list below it. The worktree panel is **default focused** when you open the dashboard.

Each worktree card shows:

- **Branch name** with ahead/behind indicators
- **Last commit message**
- **Commit age** (e.g., "2h ago", "3d ago")
- **Git status symbols** (modified, staged, untracked files)

## Actions

| Key | Action |
|-----|--------|
| `Tab` | Switch between worktree panel and MR list |
| `c` | Create a new worktree |
| `d` | Delete selected worktree |
| `M` | Merge selected worktree into the default branch |
| `Enter` | Jump to the worktree's tmux pane |

## Create workflow

When you press `c`, pertmux opens a popup dialog where you enter the branch name. It then runs `wt create` to:

1. Show an in-progress toast ("Creating worktree...")
2. Create a new worktree directory
3. Create and checkout the branch
4. Automatically refresh the dashboard (MR linking updates immediately)

## Merge workflow

Press `M` on a worktree to merge it into the default branch. pertmux runs `wt merge` which:

1. Show an in-progress toast ("Merging worktree...")
2. Merges the branch into the default branch
3. Cleans up the worktree directory
4. Removes the local branch
5. Automatically refresh the dashboard

## Split pane with agent

When the `default_agent_command` is set in your config, pressing **`Enter`** on a worktree creates a horizontal split in tmux:
- **Left pane**: Runs the specified agent command (e.g., `opencode`)
- **Right pane**: Opens an empty terminal in the worktree directory

This allows you to start working with an agent immediately upon focusing a worktree.
