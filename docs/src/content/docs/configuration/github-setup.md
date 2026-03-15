---
title: GitHub Setup
description: Configure GitHub integration with pertmux.
---

## Create a personal access token

pertmux needs a GitHub personal access token (classic) to read pull requests, check runs, and comments.

1. Go to [**Settings > Developer settings > Personal access tokens > Tokens (classic)**](https://github.com/settings/tokens)
2. Click **Generate new token (classic)**
3. Select the **`repo`** scope
4. Click **Generate token** and copy the value

The `repo` scope is required because pertmux uses the [Checks API](https://docs.github.com/en/rest/checks) to display CI/CD status, which requires full repo access even for read-only use.

For full details on managing tokens, see [GitHub's token documentation](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens).

## Add to your config

Add a `[github]` section to `~/.config/pertmux.toml`:

```toml
[github]
token = "ghp_xxxxxxxxxxxxxxxxxxxx"
```

Or use an environment variable instead:

```bash
export PERTMUX_GITHUB_TOKEN="ghp_xxxxxxxxxxxxxxxxxxxx"
```

The environment variable takes precedence over the config file value.

## Add a project

Add a `[[project]]` entry pointing to your local clone:

```toml
[github]
token = "ghp_xxxxxxxxxxxxxxxxxxxx"

[[project]]
name = "My Project"
source = "github"
project = "org/my-repo"
local_path = "/home/user/repos/my-repo"
username = "youruser"
```

| Key | Description |
|-----|-------------|
| `name` | Display name shown in the dashboard |
| `source` | Must be `"github"` |
| `project` | Full `owner/repo` path (e.g. `rupert648/pertmux`) |
| `local_path` | Absolute path to your local clone (validated at startup) |
| `username` | Your GitHub username (used to filter PRs to your own) |

## GitHub Enterprise

For GitHub Enterprise, set the `host` field to your instance hostname:

```toml
[github]
host = "github.mycompany.com"
token = "ghp_xxxxxxxxxxxxxxxxxxxx"
```

pertmux will use `https://{host}/api/v3/` for all API requests.
