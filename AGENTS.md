# pertmux: Agent Guide

This document provides a technical overview of the pertmux codebase for AI agents and developers.

## Project Overview
pertmux is a Rust TUI unified SWE dashboard that links GitLab/GitHub MRs to local branches/worktrees, tmux sessions, and coding agent instances. It provides a real-time view of session status, resource usage, and progress with integrated merge request tracking across multiple forges. The bottom panel provides worktrunk-powered worktree management with create/remove/merge actions. The architecture is pluggable — new coding agents can be added by implementing the `CodingAgent` trait, and new forges can be added by implementing the `ForgeClient` trait.

## Architecture
The project uses a **daemon/client architecture** with Unix socket IPC. A background daemon (`pertmux serve`) owns all data fetching and state, while a lightweight TUI client (`pertmux connect`) connects to render the UI.

### Daemon/Client Split
- **Daemon** (`daemon.rs`): Runs persistently in background. Owns the `App` struct (which is not `Send` due to `dyn CodingAgent`), runs on the main tokio task. Performs all data fetching on configurable timers: tmux/agent (`refresh_interval`, default 2s), MR detail (`mr_detail_interval`, default 60s), worktrees (`worktree_interval`, default 30s), MR list (`mr_list_interval`, default 300s). Listens on `/tmp/pertmux-{USER}.sock`.
- **Client** (`client.rs`): Lightweight TUI. Owns all UI state (`ClientState`: selection indices, popup state, notifications). Connects to daemon via Unix socket, receives `DashboardSnapshot` updates, sends commands (`Refresh`, `CreateWorktree`, etc.). Navigation is instant with no daemon round-trip.
- **Protocol** (`protocol.rs`): Defines `DashboardSnapshot`, `ProjectSnapshot`, `ClientMsg`, `DaemonMsg`. Framed with `LengthDelimitedCodec` + `serde_json`. Multi-client via `tokio::sync::broadcast`.

### Data Flow
1. **Daemon startup**: Loads config, validates projects, creates `App`, performs initial fetch of MRs + tmux + worktrees.
2. **Refresh loops**: Daemon runs configurable tiered timers (`refresh_interval` default 2s tmux, `mr_detail_interval` default 60s MR detail, `worktree_interval` default 30s worktrees, `mr_list_interval` default 300s MR list). After each refresh, broadcasts `DashboardSnapshot` to all connected clients.
3. **Client connect**: Connects to daemon socket. Fails with clear error if daemon not running. Receives initial snapshot immediately.
4. **Client commands**: User actions (refresh, worktree create/remove/merge, MR selection) are sent as `ClientMsg` to daemon. Daemon processes, refreshes relevant data, broadcasts updated snapshot.
5. **tmux actions**: `switch_to_pane()` and `find_or_create_pane()` run client-side — they only need data from the snapshot, not daemon state.

## Module Guide
- **main.rs**: Entry point. Uses clap for subcommands: `serve` → `daemonize()` (or `daemon::run()` with `--foreground`), `connect` → `client::run()`, `stop` → `client::stop()`, `status` → `client::status()`, `cleanup` → `client::cleanup()`. `serve` self-daemonizes by default: re-execs with `--foreground`, redirecting stdout/stderr to `/tmp/pertmux-daemon.log`, detached via `process_group(0)`. Validates config and checks for existing daemon before forking. Requires explicit subcommand (no bare `pertmux`).
- **daemon.rs**: Background daemon. Unix socket listener with `LengthDelimitedCodec` framing. Broadcast channel for multi-client snapshot fan-out. `Arc<Mutex<DashboardSnapshot>>` for latest snapshot (sent to new clients immediately). Handles `ClientMsg` commands and runs tiered refresh intervals. Tracks client count via `Arc<AtomicUsize>` and accumulates MR changes in `pending_for_offline` when no clients are connected — drains them into the initial snapshot on reconnect.
- **client.rs**: TUI client. Connects to daemon (fails with error screen if not running), owns `ClientState` with all UI state (selections, popup, notification). Event loop with `tokio::select!` on keyboard + daemon messages. Local navigation (j/k/Tab) with no round-trip. Project switching via fuzzy finder (`f` key). Also provides `stop()`, `status()`, and `cleanup()` commands. On reconnect, if `pending_changes` is non-empty, opens `ChangeSummary` modal; for live snapshots, shows toast notifications.
- **protocol.rs**: IPC protocol. Defines `DashboardSnapshot`, `ProjectSnapshot`, `GlobalMrEntry` (cross-project MR entries), `ClientMsg` (commands from client to daemon), `DaemonMsg` (responses/snapshots from daemon to client), `PROTOCOL_VERSION` for handshake validation.
- **app.rs**: Owns the `App` struct, which holds data state (panes, projects, MRs, worktrees). Manages refresh cycle, linking, and `snapshot()` method to produce `DashboardSnapshot`. UI-related methods (selection, popup) have moved to `ClientState` in `client.rs`.
- **coding_agent/mod.rs**: Defines the `CodingAgent` trait and `agents_from_config()` factory. The trait requires `name()`, `process_name()`, `query_status()`, and `send_prompt()`. Currently supports opencode and Claude Code. To add a new agent, implement the trait and register it here.
- **coding_agent/opencode.rs**: opencode implementation of `CodingAgent`. Queries `http://127.0.0.1:{port}/session/status` for session state. Requires opencode to be started with `--port 0` so it launches its HTTP server on a random port. Port discovery happens automatically via process tree inspection in `discovery.rs`.
- **coding_agent/claude_code.rs**: Claude Code implementation of `CodingAgent`. Reads JSONL transcript files from `~/.claude/projects/` and `~/.claude/transcripts/` to determine session status (Busy/Idle) and extract session details (token usage, messages, model). No HTTP server or special flags required — Claude Code writes transcripts automatically.
- **tmux.rs**: Wraps tmux CLI commands. Responsible for identifying coding agent panes (filtered by registered process names), switching focus between them, and `find_or_create_pane()` which searches all sessions for matching paths before creating new windows (prefers project-named sessions). When `default_agent_command` is configured, `find_or_create_pane()` creates a horizontal split: LEFT pane runs the agent command via `send-keys`, RIGHT pane is an empty terminal.
- **discovery.rs**: Implements port discovery. It uses `sysinfo` to find child processes and `netstat2` to map those processes to active TCP listening ports.
- **config.rs**: Defines `Config`, `AgentConfig`, `ProjectConfig`, `ProjectForge` enum, `KeybindingsConfig`, `GitLabSourceConfig`, `GitHubSourceConfig`, and per-agent config structs. Loads from TOML with `-c`/`--config` CLI flag or `~/.config/pertmux.toml`. Validates local_path existence, source configuration, token availability, project name uniqueness, and keybinding uniqueness at startup.
- **db.rs**: Manages read-only access to the opencode SQLite database. Fetches session details and enriches pane information for opencode agents.
- **types.rs**: Defines shared data structures like `AgentPane`, `SessionDetail`, and the `PaneStatus` enum.
- **ui/mod.rs**: Entry point `draw_client(frame, &ClientState)`. Constants (`ACCENT`, `NOTIFICATION_DURATION`), `ProjectRenderData` adapter, layout orchestration.
- **ui/helpers.rs**: Formatting (`truncate`, `shorten_path`, `format_tokens`), status badges, merge status display, scroll computation.
- **mr_changes.rs**: Defines `MrChange` and `MrChangeType` for tracking MR status changes. `MrChange` carries project name, MR iid/title, and change type. `MrChangeType` covers pipeline failures/successes, new discussions, and approvals. Implements `Display` for toast messages.
- **ui/components/**: Modular rendering components — `list_panel` (left panel with MR list or agent panes), `detail_panel` (right panel with MR detail or session info), `mr_sections` (MR and worktree block layouts), `cards` (individual MR/worktree cards), `overview` (project list with MR counts), `pipeline` (CI/CD dot visualization), `popup` (worktree actions, fuzzy filter, MR overview), `notification` (toast overlay — renders `ClientState.notification`), `change_summary` (reconnect modal showing accumulated MR changes).
- **worktrunk.rs**: Serde types for `wt list --format=json` output (`WtWorktree`, `WtCommit`, `WtMain`, etc.). Async functions: `fetch_worktrees()`, `create_worktree()`, `remove_worktree()`, `merge_worktree()`. Includes `format_age()` helper and 9 unit tests.
- **linking.rs**: Defines `DashboardState`, `LinkedMergeRequest`. Implements `link_all()` which connects MRs ↔ branches ↔ worktrees ↔ tmux panes ↔ Claude.
- **forge_clients/mod.rs**: Re-exports `GitLabClient` and `GitHubClient`. Sub-modules: `traits`, `types`, `gitlab`, `github`.
- **forge_clients/traits.rs**: Defines the `ForgeClient` trait with `#[async_trait(?Send)]`. Methods: `fetch_mrs()`, `fetch_mr_detail()`, `fetch_ci_jobs()`, `fetch_notes()`, `fetch_discussions()`, `fetch_user_mrs()`. All forge clients implement this trait.
- **forge_clients/types.rs**: Shared types used across all forges: `ForgeUser`, `MergeRequestSummary`, `MergeRequestDetail`, `MergeRequestNote`, `PipelineJob`, `PipelineInfo`, `UserMrSummary` (cross-project MR for global user feed).
- **forge_clients/gitlab/client.rs**: GitLab implementation of `ForgeClient`. Uses `PRIVATE-TOKEN` header auth, fetches from `/api/v4` endpoints. `fetch_ci_jobs` extracts pipeline ID from `head_pipeline`.
- **forge_clients/github/client.rs**: GitHub implementation of `ForgeClient`. Uses `Bearer` token auth with `User-Agent` header. Converts GitHub PR/check-run responses to shared types. `fetch_ci_jobs` uses `head_sha` to fetch check runs. Supports GitHub Enterprise via custom host.
- **forge_clients/github/types.rs**: Raw GitHub API response types (internal): `GhPullRequest`, `GhUser`, `GhPrRef`, `GhCheckRunsResponse`, `GhCheckRun`, `GhIssueComment`, `GhIssueItem`, `GhPullRequestStub`, `GhRepository`.
- **git.rs**: Git worktree discovery. `discover_worktrees(path)` runs `git worktree list --porcelain` and returns `Vec<WorktreeInfo>`.
- **read_state.rs**: Local SQLite DB for per-comment read/unread tracking. `ReadStateDb` tracks seen notes and MR view timestamps.

## Key Design Decisions
- **Pluggable Agents**: The `CodingAgent` trait abstracts process detection, status querying, pane enrichment, session detail fetching, and prompt delivery. Each agent handles its own discovery mechanism and communication channel internally. The `send_prompt()` trait method allows each agent to deliver prompts through its own mechanism (e.g. opencode uses its HTTP API, Claude Code uses tmux send-keys).
- **Multi-Forge Support**: `ForgeClient` trait abstracts GitLab and GitHub behind a common interface. `ProjectState.client` is `Box<dyn ForgeClient>`. Each forge handles its own API auth, response parsing, and state normalization (e.g. GitHub `"open"` → `"opened"`, check runs → pipeline jobs).
- **Multi-Project Support**: `[[project]]` TOML array with per-project forge config (`source = "gitlab"` or `"github"`), local paths, and worktree state. Fuzzy finder (`f` key) for project switching. Overview panel shows all projects with MR counts.
- **Worktrunk CLI Integration**: Uses `wt list --format=json` (NOT the library crate — author warns API is unstable). `wt` supports `-C <path>` to target specific repos. Worktree actions (create/remove/merge) via popup dialogs.
- **Optional Config**: Supports `-c`/`--config` for a TOML config file. Defaults to `~/.config/pertmux.toml`, falls back to built-in defaults if absent.
- **Startup Validation**: Config `validate()` checks local_path existence, source configuration, token availability, and project name uniqueness. Fails fast with clear error messages.
- **Read-Only DB Access**: Opens the SQLite database with `SQLITE_OPEN_READ_ONLY` to avoid locking issues or accidental corruption.
- **Smart Pane Focus**: `find_or_create_pane()` first searches ALL panes across ALL tmux sessions by `pane_current_path` (canonicalized). If no match, prefers a session whose name matches the project name (case-insensitive). Falls back to other-client heuristic, then current session. When `default_agent_command` is set, new windows are created as a horizontal split with the agent in the left pane and an empty terminal on the right; focus lands on the left (agent) pane. Without the config, behavior is a single pane (backwards compatible).
- **Responsive Layout**: The UI adapts to landscape and portrait terminal dimensions.
- **Process Tree Walking**: Port discovery relies on finding the specific child process of the tmux pane that owns the API socket.
- **MR-first layout**: When a forge (`[gitlab]` or `[github]`) is configured, the primary list entity is open MRs/PRs. Worktrees appear in a dedicated bottom section with navigation and actions.
- **Tiered refresh**: Daemon runs configurable timers — tmux/agent (`refresh_interval` default 2s), MR detail (`mr_detail_interval` default 60s), worktrees (`worktree_interval` default 30s), MR list (`mr_list_interval` default 300s). MR list also refreshed on manual 'r' or daemon startup.
- **Backwards compatibility**: No forge config (`[gitlab]`/`[github]`) = v1 behavior unchanged (agent-only mode).
- **Async runtime**: tokio + crossterm EventStream. `CodingAgent` trait stays sync (not Send) — daemon keeps `App` on main task.
- **Daemon/Client IPC**: `tokio::net::UnixStream` with `tokio_util::codec::LengthDelimitedCodec` framing and `serde_json` serialization. Multi-client via `tokio::sync::broadcast`. Client requires daemon to be running (no auto-start).
- **Socket path**: `/tmp/pertmux-{USER}.sock`. Stale socket cleaned up on daemon startup.
- **Self-daemonizing**: `pertmux serve` validates config, checks for existing daemon, then re-execs itself with `--foreground` in a new process group with stdout/stderr redirected to `/tmp/pertmux-daemon.log`. The parent prints the PID and exits immediately. `--foreground` runs the daemon in the terminal for debugging.
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
- **clap**: CLI argument parsing (subcommands: serve, connect, stop, status, cleanup).
- **toml**: Configuration file parsing.
- **anyhow**: Error handling.
- **tokio**: Async runtime (full features). Used for daemon event loop and client I/O.
- **tokio-util**: `LengthDelimitedCodec` for daemon/client IPC framing.
- **bytes**: Byte buffer for IPC messages.
- **reqwest**: Async HTTP client for forge APIs — GitLab and GitHub (json feature).
- **async-trait**: Async trait support for `ForgeClient` trait (`#[async_trait(?Send)]`).
- **futures**: StreamExt for crossterm EventStream and IPC streams.

## Landing Page & Docs Site
The `docs/` directory contains the project website: a marketing landing page + full documentation site built with Astro + Starlight + Tailwind.

### Tech Stack
- **Astro 5**: Static site generator (island architecture, near-zero JS)
- **Starlight**: Astro's docs theme (sidebar, search via Pagefind, dark/light mode)
- **Tailwind 3**: Utility CSS with custom config (`docs/tailwind.config.mjs`)
- **Fonts**: Outfit (sans), JetBrains Mono (mono) via Google Fonts

### Structure
- **`docs/src/pages/index.astro`**: Custom landing page (standalone, NOT Starlight layout)
- **`docs/src/components/`**: Landing page sections — `Navbar`, `Hero`, `LinkingDiagram`, `FeatureGrid`, `GettingStarted`, `Footer`
- **`docs/src/content/docs/`**: Markdown docs rendered by Starlight with sidebar navigation
  - `getting-started/`: Installation, Quick Start, tmux Integration
  - `configuration/`: Config Reference, Multi-Project, Forge Setup, Agent Config
  - `features/`: MR Tracking, Worktree Management, Agent Monitoring, Pipeline Visualization
  - `reference/`: Keybindings, Architecture, CLI Commands, Extending
- **`docs/src/styles/custom.css`**: Tailwind directives + Starlight theme overrides
- **`docs/astro.config.mjs`**: Starlight sidebar config, social links, custom CSS
- **`docs/tailwind.config.mjs`**: Custom colors (accent orange `#FF8C00`, gray palette), fonts, Starlight plugin

### Routes
- `/` — Landing page (custom Astro page, no Starlight layout)
- `/getting-started/installation/` — First docs page (Starlight routes docs at root, no `/docs/` prefix)
- All docs pages follow Starlight's file-based routing from `src/content/docs/`

### Build & Dev
```sh
cd docs
npm install
npm run dev       # Dev server on localhost:4321
npm run build     # Static build to docs/dist/
npm run preview   # Preview the build
```

### Visual Identity
- Dark theme primary: `#0e1015` (gray-950)
- Cards/sections: `#17191e` (gray-900)
- Accent: `#FF8C00` (orange, matches TUI `ACCENT` color)
- Screenshot placeholders exist in Hero and LinkingDiagram sections — replace with actual TUI screenshots/GIFs

### Conventions
- Landing page is a STANDALONE Astro page — does NOT use Starlight's layout or components
- Internal doc cross-links use root-relative paths (`/getting-started/quick-start/`), NOT `/docs/` prefix
- All icons are inline SVGs — no icon library dependencies
- No React/Vue/framework components — pure Astro + HTML + Tailwind

## Build & Run
- **Install**: `cargo install pertmux` (from crates.io) or `cargo install --path .` (from source)
- **Build**: `cargo build --release`
- **Start daemon**: `pertmux serve` (backgrounds automatically; `--foreground` to keep in terminal)
- **Connect client**: `pertmux connect` (daemon must be running)
- **Stop daemon**: `pertmux stop`
- **Check status**: `pertmux status`
- **Requirements**: Must run inside a tmux session. Requires coding agent instances (e.g. opencode, Claude Code) to be running in other tmux panes to display data.
- **Edition**: Rust 2024.

## CI (GitHub Actions)
Two separate workflows with path filters — only the relevant workflow runs when files change.

### Rust (`.github/workflows/rust.yml`)
Triggers on changes to `src/**`, `Cargo.toml`, `Cargo.lock`.
- **Format**: `cargo fmt --all --check`
- **Clippy**: `cargo clippy --all-targets --all-features -- -D warnings` (with `Swatinem/rust-cache`, `shared-key: bundled`)
- **Test**: `cargo test --all-features` (runs after fmt + clippy pass, shares the cache)
- Uses `dtolnay/rust-toolchain@stable` (not the unmaintained `actions-rs`)
- `rusqlite` bundled feature compiles SQLite from C source — no system deps needed on runners
- `CARGO_INCREMENTAL: 0` to avoid wasting cache space in CI

### Docs (`.github/workflows/docs.yml`)
Triggers on changes to `docs/**`.
- **Astro Check**: `npx astro check` (TypeScript diagnostics on `.astro` files, requires `@astrojs/check` + `typescript` devDependencies)
- **Build**: `npm run build` (runs after check passes)
- Node 22, `npm ci` with cache scoped to `docs/package-lock.json`
- `ASTRO_TELEMETRY_DISABLED: true` on build step

Both workflows use `concurrency` groups to cancel in-progress runs when new commits are pushed.

## Important Paths & Endpoints
- **Daemon socket**: `/tmp/pertmux-{USER}.sock`
- **Daemon log**: `/tmp/pertmux-daemon.log`
- **opencode Database**: `~/.local/share/opencode/opencode.db`
- **opencode API Endpoint**: `http://127.0.0.1:{port}/session/status`
- **Claude Code Transcripts**: `~/.claude/projects/` and `~/.claude/transcripts/`
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
- Toast notifications: Use `ClientState::notify(msg)` to show a temporary toast overlay. It sets `notification: Option<(String, Instant)>` and auto-expires after `NOTIFICATION_DURATION` (2s). The notification component (`ui/components/notification.rs`) renders it in the bottom-right corner. Use for action feedback (e.g. "Refreshing...", "Creating worktree...", "Copied: branch-name"). The daemon's `ActionResult` response also triggers a toast with the result message.
- Action keybindings are configurable via `[keybindings]` in the TOML config (e.g. `mr_overview`, `merge_worktree` which now defaults to `M`). Navigation keys (`j`/`k`/`↑`/`↓`/`Tab`/`Enter`/`Esc`/`q`) are not configurable.
- Never add AI co-author trailers (e.g. `Co-authored-by: Sisyphus ...`) to commits.
