# pertmux

pertmux ([ru]-pert multiplexer) is a unified SWE dashboard that links GitLab MRs to local branches/worktrees, tmux sessions, and coding agent instances. It provides a real-time view of merge request status, pipeline health, worktree management, and session progress — all from a single TUI.

## Features

- **GitLab MR tracking** — open MRs with pipeline dots, comments, and unread indicators
- **Worktree management** — list, create, remove, and merge worktrees via [worktrunk](https://github.com/max-sixty/worktrunk)
- **Multi-project support** — tab between projects with `h`/`l` keys
- **Smart tmux integration** — focus panes across sessions, auto-detect existing windows
- **Coding agent monitoring** — track Claude/opencode instances across tmux panes
- **Persistent popup** — runs as a tmux popup overlay via `dtach`, instant re-open

## Setup

### Prerequisites

- [tmux](https://github.com/tmux/tmux) 3.2+ (for popup support)
- [worktrunk](https://github.com/max-sixty/worktrunk) (optional) — enables the worktree management panel. Install with `cargo install worktrunk` and ensure `wt` is on your PATH.
- [dtach](https://github.com/crigler/dtach) (optional) — keeps pertmux running between popup invocations. `brew install dtach`.

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
bind-key a display-popup -h 80% -w 80% -E "dtach -A /tmp/pertmux.sock pertmux"
```

- `prefix+a` opens the dashboard as a popup overlay
- `prefix+a` again closes it (dtach detaches, pertmux keeps running)
- Next open is instant — dtach reattaches to the running process
- `q`/`Esc` fully quits (next open restarts)

Without dtach, replace with:

```tmux
bind-key a display-popup -h 80% -w 80% -E "pertmux"
```

## Configuration

pertmux works out of the box with zero configuration for basic agent monitoring. For GitLab MR tracking and multi-project support, create a TOML config file.

```
pertmux -c ./path/to/config.toml
```

If no `-c` flag is provided, pertmux looks for `~/.config/pertmux/pertmux.toml`. If that file doesn't exist, defaults are used.

### Multi-project config (recommended)

```toml
[gitlab]
host = "gitlab.example.com"
token = "glpat-your-token-here"

[[project]]
name = "My App"
source = "gitlab"
project = "team/my-app"
local_path = "/home/user/repos/my-app"
username = "youruser"

[[project]]
name = "API Service"
source = "gitlab"
project = "team/api-service"
local_path = "/home/user/repos/api-service"
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

### Agent-only config (no GitLab)

```toml
refresh_interval = 2

[agent.opencode]
# db_path = "~/.local/share/opencode/opencode.db"
```

### Config reference

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `refresh_interval` | integer | `2` | How often (in seconds) to poll tmux panes |

#### `[gitlab]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `host` | string | `gitlab.com` | GitLab instance hostname |
| `token` | string | — | Personal access token (or set `PERTMUX_GITLAB_TOKEN` env var) |

#### `[[project]]`

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `name` | string | yes | Display name (shown in tabs) |
| `source` | string | yes | `"gitlab"` (github planned) |
| `project` | string | yes | Full project path (e.g. `team/app`) |
| `local_path` | string | yes | Absolute path to local repo (validated at startup) |
| `username` | string | no | Your username (for "mine" vs "reviewing" MR grouping) |

#### `[agent.opencode]`

Including this section enables the opencode agent. Omit or comment it out to disable.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `db_path` | string | `~/.local/share/opencode/opencode.db` | Path to the opencode SQLite database |

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `j`/`k` or `↑`/`↓` | Navigate list |
| `h`/`l` or `��`/`→` | Switch project tab |
| `Tab` | Toggle between MR list and worktree panel |
| `Enter` | Focus selected pane/worktree in tmux |
| `r` | Refresh all data |

### Actions

| Key | Context | Action |
|-----|---------|--------|
| `o` | MR selected | Open MR in browser |
| `b` | Any | Copy selected branch name |
| `c` | Worktree panel | Create new worktree |
| `d` | Worktree panel | Delete selected worktree |
| `m` | Worktree panel | Merge selected worktree into default branch |
| `q`/`Esc` | Global | Quit pertmux |
| `prefix+a` | tmux | Toggle dashboard popup (dtach detaches, preserves state) |

### Pipeline Visualization

The pipeline job status dots in the MR detail panel are inspired by [glim](https://github.com/junkdog/glim). Each CI/CD job is rendered as a colored dot for a compact at-a-glance view of pipeline health, grouped by stage.
