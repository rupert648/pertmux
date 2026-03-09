---
title: Forge Setup
description: Configure GitLab, GitHub, and GitHub Enterprise integration.
---

## GitLab

### Token setup

1. Go to your GitLab instance → **Settings → Access Tokens**
2. Create a token with `read_api` scope
3. Add to your config:

```toml
[gitlab]
host = "gitlab.example.com"
token = "glpat-xxxxxxxxxxxxxxxxxxxx"
```

Or use the environment variable:

```bash
export PERTMUX_GITLAB_TOKEN="glpat-xxxxxxxxxxxxxxxxxxxx"
```

### Self-hosted GitLab

Set the `host` field to your instance hostname:

```toml
[gitlab]
host = "gitlab.mycompany.com"
```

## GitHub

### Token setup

1. Go to **GitHub → Settings → Developer Settings → Personal Access Tokens**
2. Create a token with `repo` scope (for private repos) or `public_repo` (for public only)
3. Add to your config:

```toml
[github]
token = "ghp_xxxxxxxxxxxxxxxxxxxx"
```

Or use the environment variable:

```bash
export PERTMUX_GITHUB_TOKEN="ghp_xxxxxxxxxxxxxxxxxxxx"
```

## GitHub Enterprise

Set the `host` field to your GHE hostname:

```toml
[github]
host = "github.mycompany.com"
```

pertmux will use `https://{host}/api/v3/` for API requests.
