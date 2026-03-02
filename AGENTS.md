# pertmux: Agent Guide

This document provides a technical overview of the pertmux codebase for AI agents and developers.

## Project Overview
pertmux is a Rust TUI unified SWE dashboard that links GitLab MRs to local branches/worktrees, tmux sessions, and opencode instances. It provides a real-time view of session status, resource usage, and progress with integrated GitLab merge request tracking. The architecture is pluggable — new coding agents (opencode, Claude Code, etc.) can be added by implementing the `CodingAgent` trait.

## Architecture
The project uses a tiered refresh architecture: tmux/opencode polling every 2s (sync), MR list on manual refresh, and selected MR detail every 60s (async via tokio).

### Data Flow
1. **tmux discovery**: List all tmux panes running registered coding agent processes.
2. **Agent status query**: Each agent handles its own port detection and API communication via the `CodingAgent` trait.
3. **GitLab fetch**: Query GitLab API for open merge requests, detail, and notes (async reqwest).
4. **Linking**: `link_all()` connects MRs ↔ branches ↔ worktrees ↔ tmux panes ↔ opencode instances.
5. **DB enrichment**: Query the local opencode SQLite database for session metadata (directory, tokens, messages).
6. **Read state**: Track per-comment read/unread status via local SQLite DB.
7. **TUI render**: Display the aggregated data in a responsive layout with MR-first list and detail panels.

## Module Guide
- **main.rs**: Entry point. Handles terminal initialization (raw mode, alternate screen) and the main event loop (200ms poll for input, 2s refresh).
- **app.rs**: Owns the `App` struct, which holds the entire application state. Manages the refresh cycle, selection logic, and grouping of panes by tmux session.
- **coding_agent/mod.rs**: Defines the `CodingAgent` trait and `agents_from_config()` factory. To add a new agent, implement the trait and register it here.
- **coding_agent/opencode.rs**: opencode implementation of `CodingAgent`. Handles opencode-specific HTTP API communication and status interpretation.
- **tmux.rs**: Wraps tmux CLI commands. Responsible for identifying coding agent panes (filtered by registered process names) and switching focus between them.
- **discovery.rs**: Implements port discovery. It uses `sysinfo` to find child processes and `netstat2` to map those processes to active TCP listening ports.
- **config.rs**: Defines `Config`, `AgentConfig`, and per-agent config structs. Loads from TOML with `-c`/`--config` CLI flag or `~/.config/pertmux/pertmux.toml`.
- **db.rs**: Manages read-only access to the opencode SQLite database. Fetches session details and enriches pane information.
- **types.rs**: Defines shared data structures like `AgentPane`, `SessionDetail`, and the `PaneStatus` enum.
- **ui.rs**: Contains all `ratatui` rendering logic. Separates the UI layout into a list panel (left) and a detail panel (right).
- **linking.rs**: Defines `DashboardState`, `LinkedMergeRequest`, `UnlinkedInstance`. Implements `link_all()` which connects MRs ↔ branches ↔ worktrees ↔ tmux panes ↔ opencode.
- **gitlab/mod.rs**, **gitlab/client.rs**, **gitlab/types.rs**: GitLab API client. `GitLabClient` fetches MR list, detail, and notes via reqwest. DTOs: `MergeRequestSummary`, `MergeRequestDetail`, `MergeRequestNote`.
- **git.rs**: Git worktree discovery. `discover_worktrees(path)` runs `git worktree list --porcelain` and returns `Vec<WorktreeInfo>`.
- **read_state.rs**: Local SQLite DB for per-comment read/unread tracking. `ReadStateDb` tracks seen notes and MR view timestamps.

## Key Design Decisions
- **Pluggable Agents**: The `CodingAgent` trait abstracts process detection and status querying. Each agent handles its own discovery mechanism internally.
- **Optional Config**: Supports `-c`/`--config` for a TOML config file. Defaults to `~/.config/pertmux/pertmux.toml`, falls back to built-in defaults if absent.
- **Read-Only DB Access**: Opens the SQLite database with `SQLITE_OPEN_READ_ONLY` to avoid locking issues or accidental corruption.
- **Smart Cross-Client Focus**: When focusing a pane (Enter), pertmux attempts to switch a *different* tmux client to that pane. This allows the dashboard to remain visible while the user interacts with the session.
- **Responsive Layout**: The UI adapts to landscape and portrait terminal dimensions.
- **Process Tree Walking**: Port discovery relies on finding the specific child process of the tmux pane that owns the API socket.
- **MR-first layout**: When `[gitlab]` is configured, the primary list entity is open GitLab MRs. Opencode instances without MRs appear in a separate "Unlinked" section.
- **Tiered refresh**: tmux/opencode every 2s (sync), MR list on manual 'r', selected MR detail every 60s (async reqwest).
- **Backwards compatibility**: No `[gitlab]` config = v1 behavior unchanged.
- **Async runtime**: tokio + crossterm EventStream. `CodingAgent` trait stays sync (not Send).

## Dependencies
- **ratatui**: TUI framework for rendering.
- **crossterm**: Terminal abstraction for raw mode and event handling.
- **ureq**: Minimal, synchronous HTTP client for API calls.
- **rusqlite**: SQLite bindings (using the `bundled` feature).
- **serde / serde_json**: Serialization for API responses.
- **sysinfo**: Process management and tree traversal.
- **netstat2**: Socket-to-process mapping.
- **dirs**: Cross-platform path resolution for the database location.
- **clap**: CLI argument parsing.
- **toml**: Configuration file parsing.
- **anyhow**: Error handling.
- **tokio**: Async runtime (full features).
- **reqwest**: Async HTTP client for GitLab API (json feature).
- **futures**: StreamExt for crossterm EventStream.

## Build & Run
- **Build**: `cargo build --release`
- **Requirements**: Must run inside a tmux session. Requires coding agent instances (e.g. opencode) to be running in other tmux panes to display data.
- **Edition**: Rust 2024.

## Important Paths & Endpoints
- **Database**: `~/.local/share/opencode/opencode.db`
- **API Endpoint**: `http://127.0.0.1:{port}/session/status`
- **GitLab API**: `https://{host}/api/v4/projects/{project}/merge_requests`
- **Read state DB**: `~/.local/share/pertmux/read_state.db`

## Conventions
- All application state must reside in the `App` struct.
- UI rendering logic in `ui.rs` should be pure and not trigger side effects.
- Status priority for display: Busy > Retry > Idle > Unknown.
- `link_all()` is pure logic — receives pre-fetched data, no I/O except read_state queries.
- All path comparisons use `std::fs::canonicalize()` to handle symlinks.
- GitLab token: `PERTMUX_GITLAB_TOKEN` env var overrides config file token.
