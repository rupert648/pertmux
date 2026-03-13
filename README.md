# pertmux

pertmux ([ru]-pert multiplexer) is a unified SWE dashboard that links GitLab/GitHub MRs to local branches/worktrees, tmux sessions, and coding agent instances. It provides a real-time view of merge request status, pipeline health, worktree management, and session progress — all from a single TUI.

## Features

- **Multi-forge support** — GitLab and GitHub MR/PR tracking with pipeline dots, comments, and unread indicators
- **Worktree management** — list, create, remove, and merge worktrees via [worktrunk](https://github.com/max-sixty/worktrunk)
- **Multi-project support** — fuzzy finder (`f` key) with overview panel showing MR counts
- **Smart tmux integration** — focus panes across sessions, auto-detect existing windows
- **Coding agent monitoring** — track Claude/opencode instances across tmux panes
- **Daemon/client architecture** — background daemon keeps data fresh, TUI client connects instantly via Unix socket

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    pertmux serve                        │
│                     (daemon)                            │
│                                                         │
│  ┌─────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
│  │  tmux   │  │  Forge   │  │ worktrunk│  │  agent  │ │
│  │  poll   │  │   API    │  │   CLI    │  │  status │ │
│  │  (2s)   │  │  (60s)   │  │  (30s)   │  │  (2s)   │ │
│  └────┬────┘  └────┬─────┘  └────┬─────┘  └────┬────┘ │
│       └─────────┬──┴─────────────┴──────────────┘      │
│                 ▼                                        │
│         DashboardSnapshot                               │
│                 │                                        │
│    Unix socket  │  /tmp/pertmux-{USER}.sock             │
└─────────────────┼───────────────────────────────────────┘
                  │  broadcast (multi-client)
        ┌─────────┼─────────┐
        ▼         ▼         ▼
   ┌─────────┐ ┌─────────┐ ┌─────────┐
   │ connect │ │ connect │ │ connect │
   │ (TUI)   │ │ (TUI)   │ │ (TUI)   │
   └─────────�� └─────────┘ └─────────┘
```

## Setup

### Prerequisites

- [tmux](https://github.com/tmux/tmux) 3.2+ (for popup support)
- [worktrunk](https://github.com/max-sixty/worktrunk) (optional) — enables the worktree management panel. Install with `cargo install worktrunk` and ensure `wt` is on your PATH.

### Install

```sh
cargo install --path .
```

Or build manually:

```sh
cargo build --release
# Binary at target/release/pertmux
```

### tmux Integration

Add to your `~/.tmux.conf` for a popup overlay (recommended):

```tmux
# pertmux dashboard popup (prefix+a toggles open/close)
bind-key a display-popup -h 80% -w 80% -E "pertmux connect"
```

- `prefix+a` opens the TUI client, connecting to the running daemon
- `prefix+a` again closes the popup; next open reconnects instantly
- `q`/`Esc` quits the client (daemon keeps running)

### Commands

```sh
pertmux serve              # start the background daemon
pertmux connect            # open TUI client (connects to running daemon)
pertmux stop               # stop the daemon
pertmux status             # show socket path, daemon state
pertmux --version          # show version
pertmux -c config.toml serve  # start daemon with specific config
```

The daemon must be started before connecting. It logs to `/tmp/pertmux-daemon.log` and listens on `/tmp/pertmux-{USER}.sock`.

## Configuration

pertmux works out of the box with zero configuration for basic agent monitoring. For GitLab/GitHub MR tracking and multi-project support, create a TOML config file.

```
pertmux -c ./path/to/config.toml serve
```

If no `-c` flag is provided, pertmux looks for `~/.config/pertmux/pertmux.toml`. If that file doesn't exist, defaults are used.

### Multi-project config (recommended)

```toml
[gitlab]
host = "gitlab.example.com"
token = "glpat-your-token-here"

[github]
token = "ghp_your-token-here"

[[project]]
name = "My App"
source = "gitlab"
project = "team/my-app"
local_path = "/home/user/repos/my-app"
username = "youruser"

[[project]]
name = "OSS Project"
source = "github"
project = "org/oss-project"
local_path = "/home/user/repos/oss-project"
username = "youruser"
```

### Single-project config (backwards compatible)

```toml
[gitlab]
host = "gitlab.example.com"
token = "glpat-your-token-here"
project = "team/my-app"
local_path = "/home/user/repos/my-app"
username = "youruser"
```

### Agent-only config (no forge)

```toml
refresh_interval = 2

[agent.opencode]
# db_path = "~/.local/share/opencode/opencode.db"
```

### Config reference

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `refresh_interval` | integer | `2` | How often (in seconds) to poll tmux panes |
| `default_agent_command` | string | — | Command to run in a split pane when focusing a worktree (e.g. `"opencode"`) |

#### `[gitlab]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `host` | string | `gitlab.com` | GitLab instance hostname |
| `token` | string | — | Personal access token (or set `PERTMUX_GITLAB_TOKEN` env var) |

#### `[github]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `host` | string | `github.com` | GitHub hostname (use custom host for GitHub Enterprise) |
| `token` | string | — | Personal access token (or set `PERTMUX_GITHUB_TOKEN` env var). Needs `repo` scope for private repos. |

#### `[[project]]`

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `name` | string | yes | Display name (shown in overview) |
| `source` | string | yes | `"gitlab"` or `"github"` |
| `project` | string | yes | Full project path (e.g. `team/app` or `org/repo`) |
| `local_path` | string | yes | Absolute path to local repo (validated at startup) |
| `username` | string | no | Your username (for filtering MRs/PRs to your own) |

#### `[agent.opencode]`

Including this section enables the opencode agent. Omit or comment it out to disable.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `db_path` | string | `~/.local/share/opencode/opencode.db` | Path to the opencode SQLite database |

#### `[[agent_action]]`

Define custom agent actions sent to opencode instances. When present, replaces the built-in defaults. Omit to use the two default actions (rebase, pipeline fix).

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `name` | string | yes | — | Display name in the actions popup |
| `prompt` | string | yes | — | Prompt template (supports `{target_branch}`, `{source_branch}`, `{mr_url}`, `{mr_iid}`, `{project_name}`) |
| `requires_mr` | boolean | no | `false` | If `true`, action is skipped when no MR is linked |

#### `[keybindings]`

Remap action keys. Navigation keys (`j`/`k`/`↑`/`↓`/`Tab`/`Enter`/`Esc`/`q`) are not configurable. Each action must have a unique key — duplicates are rejected at startup.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `refresh` | string | `"r"` | Refresh all data |
| `open_browser` | string | `"o"` | Open selected MR in browser |
| `copy_branch` | string | `"b"` | Copy selected branch name to clipboard |
| `filter_projects` | string | `"f"` | Fuzzy filter to switch project |
| `create_worktree` | string | `"c"` | Create new worktree |
| `delete_worktree` | string | `"d"` | Delete selected worktree |
| `merge_worktree` | string | `"m"` | Merge selected worktree into default branch |

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `j`/`k` or `↑`/`↓` | Navigate list |
| `Tab` | Toggle between MR list and worktree panel |
| `Enter` | Focus selected pane/worktree in tmux |

### Actions (configurable)

Action keys can be remapped via the `[keybindings]` section in your config file. Defaults shown below.

| Key | Context | Action |
|-----|---------|--------|
| `r` | Global | Refresh all data |
| `o` | MR selected | Open MR in browser |
| `b` | Any | Copy selected branch name |
| `f` | Global | Fuzzy filter to switch project |
| `c` | Worktree panel | Create new worktree |
| `d` | Worktree panel | Delete selected worktree |
| `m` | Worktree panel | Merge selected worktree into default branch |
| `q`/`Esc` | Global | Quit client (daemon keeps running) |
| `prefix+a` | tmux | Toggle dashboard popup |

### Pipeline Visualization

The pipeline job status dots in the MR detail panel are inspired by [glim](https://github.com/junkdog/glim). Each CI/CD job is rendered as a colored dot for a compact at-a-glance view of pipeline health, grouped by stage.
