---
title: GitLab Setup
description: Configure GitLab integration with pertmux.
---

## Create a personal access token

pertmux needs a GitLab personal access token to read merge requests, pipelines, and discussions.

1. Go to **GitLab > Edit Profile > Access Tokens** (or navigate to `/-/user_settings/personal_access_tokens`)
2. Enter a token name (e.g. `pertmux`)
3. Select the **`read_api`** scope
4. Click **Create personal access token** and copy the value

The `read_api` scope grants read-only access to the REST API, which is all pertmux needs. You do not need the full `api` scope.

For full details, see [GitLab's token documentation](https://docs.gitlab.com/user/profile/personal_access_tokens/).

## Add to your config

Add a `[gitlab]` section to `~/.config/pertmux.toml`:

```toml
[gitlab]
host = "gitlab.example.com"
token = "glpat-xxxxxxxxxxxxxxxxxxxx"
```

Or use an environment variable instead:

```bash
export PERTMUX_GITLAB_TOKEN="glpat-xxxxxxxxxxxxxxxxxxxx"
```

The environment variable takes precedence over the config file value.

## Add a project

Add a `[[project]]` entry pointing to your local clone:

```toml
[gitlab]
host = "gitlab.example.com"
token = "glpat-xxxxxxxxxxxxxxxxxxxx"

[[project]]
name = "My App"
source = "gitlab"
project = "team/my-app"
local_path = "/home/user/repos/my-app"
username = "youruser"
```

| Key | Description |
|-----|-------------|
| `name` | Display name shown in the dashboard |
| `source` | Must be `"gitlab"` |
| `project` | Full project path as shown in the URL (e.g. `team/my-app`) |
| `local_path` | Absolute path to your local clone (validated at startup) |
| `username` | Your GitLab username (used to filter MRs to your own) |

## Self-hosted GitLab

The `host` field defaults to `gitlab.com`. For self-hosted instances, set it to your instance hostname:

```toml
[gitlab]
host = "gitlab.mycompany.com"
token = "glpat-xxxxxxxxxxxxxxxxxxxx"
```

pertmux will use `https://{host}/api/v4/` for all API requests.
