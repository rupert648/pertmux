---
title: Agent Configuration
description: Configure coding agent monitoring in pertmux.
---

pertmux can monitor AI coding agent instances running in your tmux panes. Agents are enabled by including their section in the config file.

## opencode

[opencode](https://github.com/sst/opencode) is currently the only supported coding agent. The architecture is pluggable — see [Extending pertmux](/reference/extending/) and [Contributing](/reference/contributing/) if you'd like to add support for another agent.

### Requirement: `--port 0`

opencode must be started with the `--port 0` flag so it launches its local HTTP server on a random port. pertmux uses this server to query session status.

```bash
opencode --port 0
```

Without `--port 0`, opencode doesn't start its HTTP server and pertmux won't be able to detect its status.

:::tip
Add an alias to your shell profile so you don't have to remember the flag:
```bash
alias opencode='command opencode --port 0'
```
:::

### Config

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

## Agent actions

When a worktree has an active opencode session, you can press **`a`** to open the agent actions popup. This allows you to send high-level commands to the agent without leaving the dashboard.

Two built-in actions are provided by default:
- **Rebase with upstream**: Instructs the agent to rebase the current branch.
- **Check pipeline & fix**: Instructs the agent to analyze the latest pipeline failure and attempt a fix.

You can define your own custom actions via `[[agent_action]]` in your config file, with template variables like `{target_branch}` and `{mr_url}` for dynamic prompts. See [Agent Actions](/features/agent-actions/) for full details.

## Agent-only mode

If you don't need forge integration, you can run pertmux with just agent monitoring:

```toml
refresh_interval = 2

[agent.opencode]
```

This provides a dashboard of all coding agent instances across your tmux sessions without any MR tracking.

## Adding custom agents

pertmux's architecture is pluggable. New coding agents can be added by implementing the `CodingAgent` trait. See [Extending pertmux](/reference/extending/) for details.
