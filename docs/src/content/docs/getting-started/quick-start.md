---
title: Quick Start
description: Get up and running with pertmux in under 2 minutes.
---

## 1. Start the daemon

The daemon runs in the background, fetching data on tiered intervals:

```bash
pertmux serve
```

## 2. Connect the TUI

Open the dashboard in your terminal:

```bash
pertmux connect
```

That's it for basic agent monitoring. pertmux will automatically discover coding agent instances running in your tmux panes.

## 3. Add forge integration (optional)

For MR/PR tracking, create a config file at `~/.config/pertmux/pertmux.toml`:

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

Then restart the daemon:

```bash
pertmux stop
pertmux -c ~/.config/pertmux/pertmux.toml serve
```

## Next steps

- [tmux Integration](/getting-started/tmux-integration/) — Set up the popup overlay
- [Configuration](/configuration/config-reference/) — Full config reference
- [Multi-Project Setup](/configuration/multi-project/) — Track multiple repos
