---
title: Agent Actions
description: Send commands to your coding agents directly from the dashboard.
---

pertmux can send prompts to running opencode instances via the [opencode HTTP API](https://github.com/sst/opencode). This lets you trigger common workflows without switching to the agent's tmux pane.

## Prerequisites

For agent actions to work, the selected worktree needs:

1. An **opencode instance** running in a tmux pane whose working directory matches the worktree path
2. An **active session** in that opencode instance (pertmux reads the session ID from the opencode database)

If either is missing, pressing `a` shows an error toast explaining what's needed.

## Usage

1. Navigate to a worktree in the worktree panel
2. Press **`a`** to open the actions popup
3. Use **`j`/`k`** to select an action
4. Press **`Enter`** to send, or **`Esc`** to cancel

A "Sending to opencode..." toast confirms the message was dispatched. The agent processes it like any user message.

## Built-in actions

Two actions are provided by default when no `[[agent_action]]` entries are configured:

### Rebase with upstream

Sends a prompt telling the agent to rebase the current branch onto the upstream target branch. If the worktree has a linked MR, pertmux includes the MR's target branch (e.g. `main`, `develop`) in the prompt. If no MR is linked, it defaults to `main`.

### Check pipeline & fix errors

Sends a prompt telling the agent to check the CI/CD pipeline status and fix any failures. This action **requires a linked MR** — if the worktree's branch doesn't have an open MR, the action is a no-op.

The prompt includes the MR's web URL so the agent can navigate to the pipeline and inspect failing jobs.

## Custom actions

You can define your own actions via `[[agent_action]]` in your config file. When custom actions are defined, they **replace** the built-in defaults entirely.

```toml
[[agent_action]]
name = "Rebase with upstream"
prompt = "Rebase the current branch onto origin/{target_branch}. Pull the latest changes first, then rebase on top. Resolve any conflicts."

[[agent_action]]
name = "Check pipeline & fix errors"
prompt = "Check the CI/CD pipeline status for MR: {mr_url}\n\nReview any failing jobs, fix the issues, and commit."
requires_mr = true

[[agent_action]]
name = "Run tests"
prompt = "Run the full test suite for this project. Fix any failing tests."

[[agent_action]]
name = "Write PR description"
prompt = "Write a clear PR description for MR !{mr_iid} on branch {source_branch} targeting {target_branch}. Summarize the changes concisely."
requires_mr = true
```

### Template variables

Prompts support template variables that are replaced with context from the linked MR at send time:

| Variable | Description | Fallback (no MR) |
|----------|-------------|------------------|
| `{target_branch}` | MR target branch (e.g. `main`) | `main` |
| `{source_branch}` | MR source branch | empty |
| `{mr_url}` | Full web URL of the MR | empty |
| `{mr_iid}` | MR number (e.g. `42`) | empty |
| `{project_name}` | Project display name | always available |

### Fields

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `name` | string | yes | — | Display name shown in the actions popup |
| `prompt` | string | yes | — | Prompt template sent to the agent (supports template variables) |
| `requires_mr` | boolean | no | `false` | If `true`, action is skipped when no MR is linked to the worktree |

## How it works

When you confirm an action:

1. The **client** composes a prompt by substituting template variables with context from the linked MR
2. The client sends an `AgentAction` command to the daemon with the pane PID, session ID, and prompt text
3. The **daemon** discovers the opencode HTTP port via process tree walking (same mechanism used for status polling)
4. The daemon sends `POST /session/{id}/message` to the opencode API with the prompt
5. The daemon returns a success/failure toast to the client

## Configuration

The action key is configurable via `[keybindings]`:

```toml
[keybindings]
agent_actions = "a"
```

See [Keybindings](/reference/keybindings/) for all configurable keys. See [Config Reference](/configuration/config-reference/) for the full `[[agent_action]]` schema.
