---
title: Multi-Project Setup
description: Track multiple repositories across GitLab and GitHub simultaneously.
---

pertmux supports tracking multiple repositories, even across different forges. Each project gets its own MR list, worktree panel, and agent linking.

## Example config

```toml
[gitlab]
host = "gitlab.example.com"
token = "glpat-your-token-here"

[github]
token = "ghp_your-token-here"

[[project]]
name = "Backend API"
source = "gitlab"
project = "team/backend-api"
local_path = "/home/user/repos/backend-api"
username = "youruser"

[[project]]
name = "Frontend App"
source = "github"
project = "org/frontend-app"
local_path = "/home/user/repos/frontend-app"
username = "youruser"

[[project]]
name = "Shared Library"
source = "github"
project = "org/shared-lib"
local_path = "/home/user/repos/shared-lib"
username = "youruser"
```

## Switching between projects

- Press **`f`** to open the fuzzy finder and switch projects
- The **overview panel** (bottom-right) shows all projects with their MR counts
- The active project is marked with an orange `▸` indicator

## How linking works per-project

Each project independently links:

1. **MRs/PRs** from the configured forge
2. **Worktrees** discovered from the `local_path`
3. **tmux panes** matched by worktree path
4. **Agent sessions** running in matched panes

This means you can have agents working on different projects simultaneously, and pertmux will correctly link each agent to its respective MR.
