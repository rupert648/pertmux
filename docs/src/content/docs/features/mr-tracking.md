---
title: MR Tracking & Linking
description: How pertmux links merge requests to your local development environment.
---

The core innovation of pertmux is its **linking engine** — it automatically connects every layer of your development workflow.

## The linking chain

For each open MR/PR, pertmux builds a chain:

```
MR/PR → Branch → Worktree → tmux Pane → Coding Agent
```

1. **MR/PR**: Fetched from GitLab or GitHub API
2. **Branch**: The MR's source branch is matched against local branches
3. **Worktree**: If a git worktree exists for that branch, it's linked
4. **tmux Pane**: If a tmux pane's working directory matches the worktree path, it's linked
5. **Coding Agent**: If a coding agent is running in that pane, its status is shown

## What you see

For each MR in the list:

- **Title and ID** (`!142 Fix auth flow`)
- **Merge status** (mergeable, conflicts, CI pending)
- **Comment count** with unread indicator
- **Draft status**
- **Pipeline health** as colored dots
- **Agent status badge** if a coding agent is linked

## MR detail panel

Select an MR to see:

- State, branch info, author
- Detailed merge status and conflict detection
- Pipeline visualization with per-job status dots
- Linked worktree path
- Linked tmux pane and agent status
- Comment count with new activity indicator
- Last updated timestamp

## Unread tracking

pertmux tracks which comments you've seen using a local SQLite database. When new comments appear on an MR, you'll see a yellow `● new` indicator.

## Actions

| Key | Action |
|-----|--------|
| `Enter` | Jump to the linked tmux pane |
| `o` | Open MR in your browser |
| `b` | Copy branch name to clipboard |
