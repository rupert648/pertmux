---
title: Keybindings
description: Complete keybinding reference for pertmux.
---

## Navigation

| Key | Action |
|-----|--------|
| `j` / `k` or `↑` / `↓` | Navigate list |
| `f` | Fuzzy filter to switch project |
| `Tab` | Toggle between MR list and worktree panel |
| `Enter` | Focus selected pane/worktree in tmux |
| `r` | Refresh all data |

## MR actions

| Key | Context | Action |
|-----|---------|--------|
| `o` | MR selected | Open MR in browser |
| `b` | Any | Copy selected branch name to clipboard |

## Worktree actions

| Key | Context | Action |
|-----|---------|--------|
| `c` | Worktree panel | Create new worktree |
| `d` | Worktree panel | Delete selected worktree |
| `m` | Worktree panel | Merge selected worktree into default branch |

## Global

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit client (daemon keeps running) |
| `prefix+a` | Toggle dashboard popup (requires tmux config) |
