---
title: Config Reference
description: Complete configuration file reference for pertmux.
---

pertmux uses a TOML configuration file. It looks for `~/.config/pertmux/pertmux.toml` by default, or you can specify a path with `-c`:

```bash
pertmux -c ./path/to/config.toml serve
```

## Global options

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `refresh_interval` | integer | `2` | How often (in seconds) to poll tmux panes and agent status |
| `mr_detail_interval` | integer | `60` | How often (in seconds) to refresh MR detail and pipeline status |
| `worktree_interval` | integer | `30` | How often (in seconds) to refresh worktree list |
| `mr_list_interval` | integer | `300` | How often (in seconds) to refresh the MR/PR list from the forge |
| `default_agent_command` | string | — | Command to run in a split pane when focusing a worktree (e.g. `"opencode"`) |

## `[gitlab]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `host` | string | `gitlab.com` | GitLab instance hostname |
| `token` | string | — | Personal access token (or set `PERTMUX_GITLAB_TOKEN` env var) |

## `[github]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `host` | string | `github.com` | GitHub hostname (use custom host for GitHub Enterprise) |
| `token` | string | — | Personal access token (or set `PERTMUX_GITHUB_TOKEN` env var). Needs `repo` scope for private repos. |

## `[[project]]`

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `name` | string | yes | Display name (shown in overview) |
| `source` | string | yes | `"gitlab"` or `"github"` |
| `project` | string | yes | Full project path (e.g. `team/app` or `org/repo`) |
| `local_path` | string | yes | Absolute path to local repo (validated at startup) |
| `username` | string | no | Your username (for filtering MRs/PRs to your own) |

## `[agent.opencode]`

Including this section enables the opencode agent. Omit or comment it out to disable.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `db_path` | string | `~/.local/share/opencode/opencode.db` | Path to the opencode SQLite database |

## Environment variables

| Variable | Overrides | Description |
|----------|-----------|-------------|
| `PERTMUX_GITLAB_TOKEN` | `[gitlab].token` | GitLab personal access token |
| `PERTMUX_GITHUB_TOKEN` | `[github].token` | GitHub personal access token |
