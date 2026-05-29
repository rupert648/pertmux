---
title: Quick Start
description: Get up and running with pertmux in under 2 minutes.
---

## 1. Get a token

You need a personal access token from your forge:

- **GitHub**: [GitHub Setup](/configuration/github-setup/) — create a classic PAT with `repo` scope
- **GitLab**: [GitLab Setup](/configuration/gitlab-setup/) — create a token with `read_api` scope

## 2. Create a config file

Create `~/.config/pertmux.toml`:

```toml
[github]
token = "ghp_your-token-here"

[[project]]
name = "My Project"
source = "github"
project = "org/my-repo"
local_path = "/home/user/repos/my-repo"
username = "youruser"
```

See [Config Reference](/configuration/config-reference/) for all available options.

## 3. Start the daemon

```bash
pertmux serve
```

The daemon backgrounds itself automatically, polling your forge for MR/PR updates on tiered intervals.

## 4. Connect the TUI

```bash
pertmux connect
```

You should see your open MRs/PRs, linked worktrees, and any active coding agents.

:::note[Coding agent monitoring]
pertmux can monitor [opencode](https://github.com/sst/opencode), [Claude Code](https://docs.anthropic.com/en/docs/claude-code), and [Codex CLI](https://github.com/openai/codex) instances running in your tmux panes. opencode requires `--port 0`; Claude Code and Codex CLI need no special flags. See [Agent Configuration](/configuration/agent-config/) for setup details.

If you just want agent monitoring without forge integration, skip steps 1-2 — pertmux will auto-discover agent instances in your tmux panes.
:::

## Next steps

- [tmux Integration](/getting-started/tmux-integration/) — Set up the popup overlay
- [Multi-Project Setup](/configuration/multi-project/) — Track multiple repos
- [Worktree Management](/features/worktree-management/) — Create and manage worktrees from the dashboard
