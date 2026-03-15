---
title: Security & Privacy
description: How pertmux handles your data and credentials.
---

pertmux is a local-only tool. It acts as a read-only wrapper around tmux, your forge's REST API, and local coding agent servers.

## Your tokens

Personal access tokens (configured in `~/.config/pertmux.toml` or via environment variables) are used **only** to call the GitHub or GitLab API endpoints you configured. pertmux never stores, transmits, or exposes your tokens beyond these API calls.

## Your data

All data stays on your machine:

- **MR/PR data** is fetched from your forge's API and held in memory by the daemon. Nothing is written to disk except the read-state database (`~/.local/share/pertmux/read_state.db`), which tracks which comments you've seen.
- **Agent data** is read from the local opencode SQLite database and the agent's local HTTP server. No data is sent externally.
- **Worktree data** comes from local `git` and `wt` CLI commands.

## Network access

pertmux makes outbound HTTPS requests to exactly two destinations:

1. **Your forge API** — `https://{gitlab-host}/api/v4/` or `https://{github-host}/api/v3/` (or `api.github.com`)
2. **Local agent server** — `http://127.0.0.1:{port}/session/status` (localhost only)

There is no telemetry, no analytics, no phoning home. pertmux makes zero network requests beyond what you explicitly configure.

## Source code

pertmux is fully open source. You can audit every network call in the codebase:

- Forge API calls: `src/forge_clients/gitlab/client.rs` and `src/forge_clients/github/client.rs`
- Agent API calls: `src/coding_agent/opencode.rs`
- Port discovery: `src/discovery.rs` (local process inspection only)
