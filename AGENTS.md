# pertmux: Agent Guide

This document provides a technical overview of the pertmux codebase for AI agents and developers.

## Project Overview
pertmux is a Rust TUI unified SWE dashboard that links GitLab/GitHub MRs to local branches/worktrees, tmux sessions, and coding agent instances. It provides a real-time view of session status, resource usage, and progress with integrated merge request tracking across multiple forges. The bottom panel provides worktrunk-powered worktree management with create/remove/merge actions. The architecture is pluggable — new coding agents can be added by implementing the `CodingAgent` trait, and new forges can be added by implementing the `ForgeClient` trait.

## Architecture
The project uses a **daemon/client architecture** with Unix socket IPC. A background daemon (`pertmux serve`) owns all data fetching and state, while a lightweight TUI client (`pertmux connect`) connects to render the UI.

### Daemon/Client Split
- **Daemon** (`daemon.rs`): Runs persistently in background. Owns the `App` struct (which is not `Send` due to `dyn CodingAgent`), runs on the main tokio task. Performs all data fetching on timers: tmux/agent every 2s, MR detail every 60s, worktrees every 30s. Listens on `/tmp/pertmux-{USER}.sock`.
- **Client** (`client.rs`): Lightweight TUI. Owns all UI state (`ClientState`: selection indices, popup state, notifications). Connects to daemon via Unix socket, receives `DashboardSnapshot` updates, sends commands (`Refresh`, `CreateWorktree`, etc.). Navigation is instant with no daemon round-trip.
- **Protocol** (`protocol.rs`): Defines `DashboardSnapshot`, `ProjectSnapshot`, `ClientMsg`, `DaemonMsg`. Framed with `LengthDelimitedCodec` + `serde_json`. Multi-client via `tokio::sync::broadcast`.

### Data Flow
1. **Daemon startup**: Loads config, validates projects, creates `App`, performs initial fetch of MRs + tmux + worktrees.
2. **Refresh loops**: Daemon runs tiered timers (2s tmux, 60s MR detail, 30s worktrees). After each refresh, broadcasts `DashboardSnapshot` to all connected clients.
3. **Client connect**: Connects to daemon socket. Fails with clear error if daemon not running. Receives initial snapshot immediately.
4. **Client commands**: User actions (refresh, worktree create/remove/merge, MR selection) are sent as `ClientMsg` to daemon. Daemon processes, refreshes relevant data, broadcasts updated snapshot.
5. **tmux actions**: `switch_to_pane()` and `find_or_create_pane()` run client-side — they only need data from the snapshot, not daemon state.

## Module Guide
- **main.rs**: Entry point. Uses clap for subcommands: `serve` → `daemon::run()`, `connect` → `client::run()`, `stop` → `client::stop()`, `status` → `client::status()`. Requires explicit subcommand (no bare `pertmux`).
- **daemon.rs**: Background daemon. Unix socket listener with `LengthDelimitedCodec` framing. Broadcast channel for multi-client snapshot fan-out. `Arc<Mutex<DashboardSnapshot>>` for latest snapshot (sent to new clients immediately). Handles `ClientMsg` commands and runs tiered refresh intervals.
- **client.rs**: TUI client. Connects to daemon (fails with error screen if not running), owns `ClientState` with all UI state (selections, popup, notification). Event loop with `tokio::select!` on keyboard + daemon messages. Local navigation (j/k/Tab) with no round-trip. Project switching via fuzzy finder (`f` key). Also provides `stop()` and `status()` commands.
- **protocol.rs**: IPC protocol. `DashboardSnapshot`, `ProjectSnapshot` (the serialization boundary), `ClientMsg` (commands from client to daemon), `DaemonMsg` (responses/snapshots from daemon to client), `PROTOCOL_VERSION` for handshake validation.
- **app.rs**: Owns the `App` struct, which holds data state (panes, projects, MRs, worktrees). Manages refresh cycle, linking, and `snapshot()` method to produce `DashboardSnapshot`. UI-related methods (selection, popup) have moved to `ClientState` in `client.rs`.
- **coding_agent/mod.rs**: Defines the `CodingAgent` trait and `agents_from_config()` factory. To add a new agent, implement the trait and register it here.
- **coding_agent/Claude.rs**: Claude implementation of `CodingAgent`. Handles Claude-specific HTTP API communication and status interpretation.
- **tmux.rs**: Wraps tmux CLI commands. Responsible for identifying coding agent panes (filtered by registered process names), switching focus between them, and `find_or_create_pane()` which searches all sessions for matching paths before creating new windows (prefers project-named sessions).
- **discovery.rs**: Implements port discovery. It uses `sysinfo` to find child processes and `netstat2` to map those processes to active TCP listening ports.
- **config.rs**: Defines `Config`, `AgentConfig`, `ProjectConfig`, `ProjectForge` enum, `GitLabSourceConfig`, `GitHubSourceConfig`, and per-agent config structs. Loads from TOML with `-c`/`--config` CLI flag or `~/.config/pertmux/pertmux.toml`. Validates local_path existence, source configuration, token availability, and project name uniqueness at startup.
- **db.rs**: Manages read-only access to the Claude SQLite database. Fetches session details and enriches pane information.
- **types.rs**: Defines shared data structures like `AgentPane`, `SessionDetail`, and the `PaneStatus` enum.
- **ui/mod.rs**: Entry point `draw_client(frame, &ClientState)`. Constants (`ACCENT`, `NOTIFICATION_DURATION`), `ProjectRenderData` adapter, layout orchestration.
- **ui/helpers.rs**: Formatting (`truncate`, `shorten_path`, `format_tokens`), status badges, merge status display, scroll computation.
- **ui/components/**: Modular rendering components — `list_panel` (left panel with MR list or agent panes), `detail_panel` (right panel with MR detail or session info), `mr_sections` (MR and worktree block layouts), `cards` (individual MR/worktree cards), `overview` (project list with MR counts), `pipeline` (CI/CD dot visualization), `popup` (worktree actions and fuzzy filter), `notification` (toast overlay).
- **worktrunk.rs**: Serde types for `wt list --format=json` output (`WtWorktree`, `WtCommit`, `WtMain`, etc.). Async functions: `fetch_worktrees()`, `create_worktree()`, `remove_worktree()`, `merge_worktree()`. Includes `format_age()` helper and 9 unit tests.
- **linking.rs**: Defines `DashboardState`, `LinkedMergeRequest`. Implements `link_all()` which connects MRs ↔ branches ↔ worktrees ↔ tmux panes ↔ Claude.
- **forge_clients/mod.rs**: Re-exports `GitLabClient` and `GitHubClient`. Sub-modules: `traits`, `types`, `gitlab`, `github`.
- **forge_clients/traits.rs**: Defines the `ForgeClient` trait with `#[async_trait(?Send)]`. Methods: `fetch_mrs()`, `fetch_mr_detail()`, `fetch_ci_jobs()`, `fetch_notes()`. All forge clients implement this trait.
- **forge_clients/types.rs**: Shared types used across all forges: `ForgeUser`, `MergeRequestSummary`, `MergeRequestDetail`, `MergeRequestNote`, `PipelineJob`, `PipelineInfo`.
- **forge_clients/gitlab/client.rs**: GitLab implementation of `ForgeClient`. Uses `PRIVATE-TOKEN` header auth, fetches from `/api/v4` endpoints. `fetch_ci_jobs` extracts pipeline ID from `head_pipeline`.
- **forge_clients/github/client.rs**: GitHub implementation of `ForgeClient`. Uses `Bearer` token auth with `User-Agent` header. Converts GitHub PR/check-run responses to shared types. `fetch_ci_jobs` uses `head_sha` to fetch check runs. Supports GitHub Enterprise via custom host.
- **forge_clients/github/types.rs**: Raw GitHub API response types (internal): `GhPullRequest`, `GhUser`, `GhPrRef`, `GhCheckRunsResponse`, `GhCheckRun`, `GhIssueComment`.
- **git.rs**: Git worktree discovery. `discover_worktrees(path)` runs `git worktree list --porcelain` and returns `Vec<WorktreeInfo>`.
- **read_state.rs**: Local SQLite DB for per-comment read/unread tracking. `ReadStateDb` tracks seen notes and MR view timestamps.

## Key Design Decisions
- **Pluggable Agents**: The `CodingAgent` trait abstracts process detection and status querying. Each agent handles its own discovery mechanism internally.
- **Multi-Forge Support**: `ForgeClient` trait abstracts GitLab and GitHub behind a common interface. `ProjectState.client` is `Box<dyn ForgeClient>`. Each forge handles its own API auth, response parsing, and state normalization (e.g. GitHub `"open"` → `"opened"`, check runs → pipeline jobs).
- **Multi-Project Support**: `[[project]]` TOML array with per-project forge config (`source = "gitlab"` or `"github"`), local paths, and worktree state. Fuzzy finder (`f` key) for project switching. Overview panel shows all projects with MR counts.
- **Worktrunk CLI Integration**: Uses `wt list --format=json` (NOT the library crate — author warns API is unstable). `wt` supports `-C <path>` to target specific repos. Worktree actions (create/remove/merge) via popup dialogs.
- **Optional Config**: Supports `-c`/`--config` for a TOML config file. Defaults to `~/.config/pertmux/pertmux.toml`, falls back to built-in defaults if absent.
- **Startup Validation**: Config `validate()` checks local_path existence, source configuration, token availability, and project name uniqueness. Fails fast with clear error messages.
- **Read-Only DB Access**: Opens the SQLite database with `SQLITE_OPEN_READ_ONLY` to avoid locking issues or accidental corruption.
- **Smart Pane Focus**: `find_or_create_pane()` first searches ALL panes across ALL tmux sessions by `pane_current_path` (canonicalized). If no match, prefers a session whose name matches the project name (case-insensitive). Falls back to other-client heuristic, then current session.
- **Responsive Layout**: The UI adapts to landscape and portrait terminal dimensions.
- **Process Tree Walking**: Port discovery relies on finding the specific child process of the tmux pane that owns the API socket.
- **MR-first layout**: When a forge (`[gitlab]` or `[github]`) is configured, the primary list entity is open MRs/PRs. Worktrees appear in a dedicated bottom section with navigation and actions.
- **Tiered refresh**: Daemon runs timers — tmux/agent every 2s, MR detail every 60s, worktrees every 30s. MR list refreshed on manual 'r' or daemon startup.
- **Backwards compatibility**: No forge config (`[gitlab]`/`[github]`) = v1 behavior unchanged (agent-only mode).
- **Async runtime**: tokio + crossterm EventStream. `CodingAgent` trait stays sync (not Send) — daemon keeps `App` on main task.
- **Daemon/Client IPC**: `tokio::net::UnixStream` with `tokio_util::codec::LengthDelimitedCodec` framing and `serde_json` serialization. Multi-client via `tokio::sync::broadcast`. Client requires daemon to be running (no auto-start).
- **Socket path**: `/tmp/pertmux-{USER}.sock`. Stale socket cleaned up on daemon startup.
- **Daemon lifecycle**: Runs until killed or `pertmux stop`. No idle timeout. Single daemon per user.

## Dependencies
- **ratatui**: TUI framework for rendering.
- **crossterm**: Terminal abstraction for raw mode and event handling.
- **ureq**: Minimal, synchronous HTTP client for agent API calls.
- **rusqlite**: SQLite bindings (using the `bundled` feature).
- **serde / serde_json**: Serialization for API responses, worktrunk JSON, and daemon/client IPC.
- **sysinfo**: Process management and tree traversal.
- **netstat2**: Socket-to-process mapping.
- **dirs**: Cross-platform path resolution for the database location.
- **clap**: CLI argument parsing (subcommands: serve, connect, stop, status).
- **toml**: Configuration file parsing.
- **anyhow**: Error handling.
- **tokio**: Async runtime (full features). Used for daemon event loop and client I/O.
- **tokio-util**: `LengthDelimitedCodec` for daemon/client IPC framing.
- **bytes**: Byte buffer for IPC messages.
- **reqwest**: Async HTTP client for forge APIs — GitLab and GitHub (json feature).
- **async-trait**: Async trait support for `ForgeClient` trait (`#[async_trait(?Send)]`).
- **futures**: StreamExt for crossterm EventStream and IPC streams.

## Build & Run
- **Build**: `cargo build --release`
- **Start daemon**: `pertmux serve`
- **Connect client**: `pertmux connect` (daemon must be running)
- **Stop daemon**: `pertmux stop`
- **Check status**: `pertmux status`
- **Requirements**: Must run inside a tmux session. Requires coding agent instances (e.g. opencode) to be running in other tmux panes to display data.
- **Edition**: Rust 2024.

## Important Paths & Endpoints
- **Daemon socket**: `/tmp/pertmux-{USER}.sock`
- **Daemon log**: `/tmp/pertmux-daemon.log`
- **Database**: `~/.local/share/opencode/opencode.db`
- **API Endpoint**: `http://127.0.0.1:{port}/session/status`
- **GitLab API**: `https://{host}/api/v4/projects/{project}/merge_requests`
- **GitHub API**: `https://api.github.com/repos/{owner}/{repo}/pulls` (or `https://{host}/api/v3/` for GHE)
- **Read state DB**: `~/.local/share/pertmux/read_state.db`

## Conventions
- Data state (panes, projects, MRs) resides in the `App` struct (daemon-side). UI state (selection, popup, notification) resides in `ClientState` (client-side).
- UI rendering logic in `ui.rs` should be pure and not trigger side effects.
- Status priority for display: Busy > Retry > Idle > Unknown.
- `link_all()` is pure logic — receives pre-fetched data, no I/O except read_state queries.
- All path comparisons use `std::fs::canonicalize()` to handle symlinks.
- GitLab token: `PERTMUX_GITLAB_TOKEN` env var overrides config file token. GitHub token: `PERTMUX_GITHUB_TOKEN`.
- `ProjectForge` is an enum (`Gitlab`, `Github`) — not a string. Validated at parse time.
- Worktrunk integration uses CLI wrapper only (`wt list --format=json`), NOT the library crate.
- Do NOT use `--full` or `--branches` flags on `wt list` (adds network calls).
- Do NOT use `statusline` field from wt output (contains ANSI escape codes). Use `symbols` field instead.
- No unsafe code. Manual validation with `anyhow` (no validation crate).
- `ACCENT` color constant: `Color::Rgb(255, 140, 0)` (orange).
