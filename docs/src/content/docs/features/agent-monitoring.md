---
title: Agent Monitoring
description: Monitor AI coding agents running across your tmux sessions.
---

pertmux detects and monitors AI coding agent instances running in tmux panes across all your sessions.

## Supported agents

pertmux supports two coding agents:

- **[opencode](https://github.com/sst/opencode)** — must be started with `--port 0` so pertmux can query its local HTTP server. Status is detected via HTTP API.
- **[Claude Code](https://docs.anthropic.com/en/docs/claude-code)** — requires no special flags. Status is detected by reading JSONL transcript files from `~/.claude/`.

See [Agent Configuration](/configuration/agent-config/) for setup details.

The architecture is pluggable — new agents can be added by implementing the `CodingAgent` trait. See [Extending pertmux](/reference/extending/) and [Contributing](/reference/contributing/).

## How detection works

Every 2 seconds (configurable via `refresh_interval`), the daemon:

1. Lists all tmux panes across all sessions
2. Checks each pane's running process against registered agent process names (`opencode`, `claude`)
3. For matched panes, queries the agent for status using its own mechanism:
   - **opencode**: Discovers the HTTP server port via process tree inspection and queries the API
   - **Claude Code**: Reads JSONL transcript files from `~/.claude/` and infers status from the last entry
4. Enriches each pane with session details (title, model, tokens, messages)
5. Links each agent pane to its corresponding MR via the worktree path

## Agent status

Each detected agent shows a status badge:

| Status | Meaning |
|--------|---------|
| **Busy** | Agent is actively working (generating code, running tools) |
| **Idle** | Agent has finished its current task |
| **Retry** | Agent encountered an error and is retrying |
| **Unknown** | Status could not be determined |

Status priority for display: Busy > Retry > Idle > Unknown.

## Agent Actions

Press **`a`** on a worktree with an active agent session to send commands to the agent — rebase, fix pipeline failures, and more. Actions are delivered via HTTP API for opencode and via tmux send-keys for Claude Code. See [Agent Actions](/features/agent-actions/) for details.

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
