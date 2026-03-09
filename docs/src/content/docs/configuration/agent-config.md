---
title: Agent Configuration
description: Configure coding agent monitoring in pertmux.
---

pertmux can monitor AI coding agent instances running in your tmux panes. Agents are enabled by including their section in the config file.

## opencode

```toml
[agent.opencode]
db_path = "~/.local/share/opencode/opencode.db"
```

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `db_path` | string | `~/.local/share/opencode/opencode.db` | Path to the opencode SQLite database |

### What it shows

When an opencode agent is detected in a tmux pane, pertmux displays:

- **Status**: Busy, Idle, Retry, or Unknown
- **Session title**: The active session name
- **Token usage**: Input and output token counts
- **Message count**: Total messages in the session
- **Todo list**: The agent's current task progress
- **Message timeline**: Recent conversation history

## Agent-only mode

If you don't need forge integration, you can run pertmux with just agent monitoring:

```toml
refresh_interval = 2

[agent.opencode]
```

This provides a dashboard of all coding agent instances across your tmux sessions without any MR tracking.

## Adding custom agents

pertmux's architecture is pluggable. New coding agents can be added by implementing the `CodingAgent` trait. See [Extending pertmux](/reference/extending/) for details.
