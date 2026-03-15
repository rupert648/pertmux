---
title: tmux Integration
description: Set up pertmux as a tmux popup overlay for instant access.
---

## Popup overlay (recommended)

Add to your `~/.tmux.conf`:

```tmux
# pertmux dashboard popup (prefix+a toggles open/close)
bind-key a display-popup -h 80% -w 80% -E "pertmux connect"
```

This gives you:

- **`prefix+a`** opens the TUI client, connecting to the running daemon
- **`prefix+a`** again closes the popup; next open reconnects instantly
- **`q`/`Esc`** quits the client (daemon keeps running in the background)

## How it works

The daemon (`pertmux serve`) runs persistently and keeps data fresh. Each time you open the popup, `pertmux connect` attaches to the daemon via Unix socket and receives the latest `DashboardSnapshot` immediately — no loading delay.

## Smart pane focus

When you press `Enter` on an MR or worktree, pertmux uses smart pane focus to jump to the right tmux pane:

1. Searches ALL panes across ALL tmux sessions by matching the worktree path
2. If no match, prefers a session whose name matches the project name
3. Falls back to the current session

This means you can have coding agents scattered across multiple tmux sessions and pertmux will always find them.

## Session-per-project workflow

pertmux works best when you use **one tmux session per project**. When you press `Enter` on a worktree, pertmux looks for a tmux session whose name matches the project name (case-insensitive). If it finds one, it opens the worktree as a new window in that session. If no matching session exists, it falls back to opening a window in your current session instead.

This means your tmux session list naturally mirrors your active projects — each session has one window per worktree, and pertmux keeps everything organized automatically.

Here's a trimmed tmux config that supports this workflow well:

```tmux
# Start windows from 1
set -g base-index 1
setw -g pane-base-index 1

# Quick window cycling
bind -n C-n next-window
bind -n C-p previous-window

# Session navigation — flip between project sessions
bind J switch-client -n
bind K switch-client -p

# Create a new session by name (use your project name)
bind S command-prompt -p "New Session:" "new-session -A -s '%%'"

# pertmux dashboard popup
bind-key a display-popup -h 80% -w 80% -E "pertmux connect"
```

Typical flow:

1. **Start sessions** for each project: `prefix+S` → type `my-app`, `prefix+S` → type `oss-lib`
2. **Open pertmux** with `prefix+a`, navigate to a worktree, press `Enter`
3. pertmux opens the worktree as a new window in the matching session
4. **Flip between projects** with `prefix+J` / `prefix+K` — each session has its own set of worktree windows
