---
title: Agent Monitoring
description: Monitor AI coding agents running across your tmux sessions.
---

pertmux detects and monitors AI coding agent instances running in tmux panes across all your sessions.

## How detection works

Every 2 seconds, the daemon:

1. Lists all tmux panes across all sessions
2. Checks each pane's running process against registered agent process names
3. For matched panes, queries the agent's API or database for session details
4. Links each agent pane to its corresponding MR via the worktree path

## Agent status

Each detected agent shows a status badge:

| Status | Meaning |
|--------|---------|
| **Busy** | Agent is actively working (generating code, running tools) |
| **Idle** | Agent has finished its current task |
| **Retry** | Agent encountered an error and is retrying |
| **Unknown** | Status could not be determined |

Status priority for display: Busy > Retry > Idle > Unknown.

## Session details

When you select an agent pane, the detail panel shows:

- **Working directory**
- **Token usage** (input and output tokens)
- **Message count** and session duration
- **File changes** (files modified, additions, deletions)
- **Todo list** with completion status
- **Message timeline** with role indicators and text previews

## Agent-only mode

If you don't configure any forge (`[gitlab]` or `[github]`), pertmux runs in agent-only mode — showing a simple list of all detected coding agents grouped by tmux session.
