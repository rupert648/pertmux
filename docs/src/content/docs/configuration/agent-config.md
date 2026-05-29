---
title: Agent Configuration
description: Configure coding agent monitoring in pertmux.
---

pertmux can monitor AI coding agent instances running in your tmux panes. Agents are enabled by including their section in the config file.

## opencode

[opencode](https://github.com/sst/opencode) is a supported coding agent. The architecture is pluggable — see [Extending pertmux](/reference/extending/) and [Contributing](/reference/contributing/) if you'd like to add support for another agent.

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

## Claude Code

[Claude Code](https://docs.anthropic.com/en/docs/claude-code) is Anthropic's CLI coding agent. Unlike opencode, Claude Code requires **no special startup flags** — pertmux reads its JSONL transcript files automatically.

### No special flags needed

Just run Claude Code normally:

```bash
claude
```

pertmux automatically finds Claude Code's transcript files in `~/.claude/projects/` and `~/.claude/transcripts/` to determine session status and details.

### Config

```toml
[agent.claude_code]
```

No configuration options are needed — Claude Code uses `~/.claude/` by default and pertmux discovers transcripts automatically.

### How it works

Claude Code writes session data as JSONL (JSON Lines) transcript files. pertmux reads these files to determine:

- **Status**: Inferred from the last transcript entry
  - `user` or `tool_use` entry → **Busy** (Claude is working)
  - `assistant` or `tool_result` entry → **Idle** (Claude has finished)
- **Session details**: Parsed from the JSONL entries including model, timestamps, token usage
- **Message timeline**: Built from the transcript entries

### What it shows

When a Claude Code agent is detected in a tmux pane, pertmux displays:

- **Status**: Busy, Idle, or Unknown
- **Session title**: The first user message (truncated)
- **Token usage**: Cumulative input and output token counts (including cache tokens)
- **Message count**: Total entries in the session
- **Model**: The Claude model being used (e.g. `claude-sonnet-4-6`)
- **Message timeline**: Recent conversation turns

### Process detection

Claude Code appears as the `claude` process in tmux panes. pertmux matches this process name to identify Claude Code instances across all your tmux sessions.

## Codex CLI

[Codex CLI](https://github.com/openai/codex) is OpenAI's terminal coding agent. Like Claude Code, it requires **no special startup flags** — pertmux reads its local SQLite databases automatically.

### No special flags needed

Just run Codex normally:

```bash
codex
```

pertmux reads Codex's local databases in `~/.codex/` to determine session status and details.

### Config

```toml
[agent.codex]
```

No configuration options are needed by default. Optionally override the Codex home directory:

```toml
[agent.codex]
codex_home = "/custom/path/to/.codex"
```

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `codex_home` | string | `~/.codex` | Override the Codex home directory |

### How it works

Codex CLI stores session data in two SQLite databases under `~/.codex/`:

- **`state_5.sqlite`** — Thread metadata (title, model, cwd, token usage, timestamps)
- **`logs_2.sqlite`** — Trace-level log entries used for busy/idle detection

pertmux matches a tmux pane's working directory to a thread's `cwd` in the state database, then checks recent log entries to determine status:

- Recent `codex.op="user_input"` span with no subsequent completion → **Busy**
- Recent `codex.op="interrupt"` or turn completion → **Idle**
- No matching thread → **Unknown**

### What it shows

When a Codex agent is detected in a tmux pane, pertmux displays:

- **Status**: Busy, Idle, or Unknown
- **Session title**: The first user message (truncated)
- **Token usage**: Cumulative token count from the thread
- **Model**: The model being used (e.g. `gpt-5.4`)
- **Message timeline**: Reconstructed from log entries

### Process detection

Codex CLI appears as the `codex` process in tmux panes. pertmux matches this process name to identify Codex instances across all your tmux sessions.

## Agent actions

When a worktree has an active agent session, you can press **`a`** to open the agent actions popup. This allows you to send high-level commands to the agent without leaving the dashboard.

Two built-in actions are provided by default:
- **Rebase with upstream**: Instructs the agent to rebase the current branch.
- **Check pipeline & fix**: Instructs the agent to analyze the latest pipeline failure and attempt a fix.

### How actions are delivered

- **opencode**: Actions are sent via HTTP POST to opencode's local API (`/session/{id}/message`)
- **Claude Code**: Actions are sent via `tmux send-keys` — the prompt is typed directly into the Claude Code terminal
- **Codex CLI**: Actions are sent via `tmux send-keys` — the prompt is typed directly into the Codex terminal

You can define your own custom actions via `[[agent_action]]` in your config file, with template variables like `{target_branch}` and `{mr_url}` for dynamic prompts. See [Agent Actions](/features/agent-actions/) for full details.

## Agent-only mode

If you don't need forge integration, you can run pertmux with just agent monitoring:

```toml
refresh_interval = 2

[agent.opencode]

[agent.claude_code]

[agent.codex]
```

This provides a dashboard of all coding agent instances across your tmux sessions without any MR tracking. You can enable any combination of agents.

## Adding custom agents

pertmux's architecture is pluggable. New coding agents can be added by implementing the `CodingAgent` trait. See [Extending pertmux](/reference/extending/) for details.
