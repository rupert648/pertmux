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
