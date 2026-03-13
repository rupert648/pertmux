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

## `[keybindings]`

Remap action keys. Navigation keys (`j`/`k`/`↑`/`↓`/`Tab`/`Enter`/`Esc`/`q`) are not configurable. Each action must have a unique key — duplicates are rejected at startup with a clear error message.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `refresh` | string | `"r"` | Refresh all data |
| `open_browser` | string | `"o"` | Open selected MR in browser |
| `copy_branch` | string | `"b"` | Copy selected branch name to clipboard |
| `filter_projects` | string | `"f"` | Fuzzy filter to switch project |
| `create_worktree` | string | `"c"` | Create new worktree |
| `delete_worktree` | string | `"d"` | Delete selected worktree |
| `merge_worktree` | string | `"m"` | Merge selected worktree into default branch |
| `agent_actions` | string | `"a"` | Open agent actions panel |

## `[[agent_action]]`

Define custom agent actions that can be sent to opencode instances from the dashboard. When any `[[agent_action]]` entries are present, they replace the built-in defaults. Omit this section entirely to use the two default actions (rebase, pipeline fix).

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `name` | string | yes | — | Display name shown in the actions popup |
| `prompt` | string | yes | — | Prompt template sent to the agent |
| `requires_mr` | boolean | no | `false` | If `true`, action is skipped when no MR is linked |

Prompts support template variables: `{target_branch}`, `{source_branch}`, `{mr_url}`, `{mr_iid}`, `{project_name}`. See [Agent Actions](/features/agent-actions/) for details and examples.

## Environment variables

| Variable | Overrides | Description |
|----------|-----------|-------------|
| `PERTMUX_GITLAB_TOKEN` | `[gitlab].token` | GitLab personal access token |
| `PERTMUX_GITHUB_TOKEN` | `[github].token` | GitHub personal access token |
