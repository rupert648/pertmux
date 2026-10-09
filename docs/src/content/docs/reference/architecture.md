---
title: Architecture
description: How pertmux's daemon/client architecture works.
---

pertmux uses a daemon/client architecture with Unix socket IPC.

## Overview

```
┌─────────────────────────────────────────────────────────┐
│                    pertmux serve                        │
│                     (daemon)                            │
│                                                         │
│  ┌─────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
│  │  tmux   │  │  Forge   │  │ worktrunk│  │  agent  │ │
│  │  poll   │  │   API    │  │   CLI    │  │  status │ │
│  │  (2s)   │  │  (60s)   │  │  (30s)   │  │  (2s)   │ │
│  └────┬────┘  └────┬─────┘  └────┬─────┘  └────┬────┘ │
│       └─────────┬──┴─────────────┴──────────────┘      │
│                 ▼                                        │
│         DashboardSnapshot                               │
│                 │                                        │
│    Unix socket  │  /tmp/pertmux-{USER}.sock             │
└─────────────────┼───────────────────────────────────────┘
                  │  broadcast (multi-client)
        ┌─────────┼─────────┐
        ▼         ▼         ▼
   ┌─────────┐ ┌─────────┐ ┌─────────┐
   │ connect │ │ connect │ │ connect │
   │ (TUI)   │ │ (TUI)   │ │ (TUI)   │
   └─────────┘ └─────────┘ └─────────┘
```

## Daemon

The daemon (`pertmux serve`) runs persistently in the background. It:

- Owns all data state (panes, projects, MRs, worktrees)
- Runs tiered refresh intervals: tmux/agent every 2s, worktrees every 30s, MR details every 60s
- Listens on `/tmp/pertmux-{USER}.sock`
- Broadcasts `DashboardSnapshot` to all connected clients via `tokio::sync::broadcast`
- Processes client commands (refresh, worktree actions, etc.)
- Processes `CodexHook` messages from `pertmux codex-hook` for immediate Codex status hints

## Client

The TUI client (`pertmux connect`) is lightweight. It:

- Owns only UI state (selection indices, popup state, notifications)
- Connects to the daemon via Unix socket
- Receives `DashboardSnapshot` updates and renders them
- Sends commands back to the daemon for data operations
- Navigation is instant with no daemon round-trip

## Protocol

Communication uses `LengthDelimitedCodec` framing with `serde_json` serialization:

- **`DashboardSnapshot`**: Full state snapshot sent from daemon to client
- **`ClientMsg`**: Commands from client to daemon (Refresh, CreateWorktree, etc.)
- **`DaemonMsg`**: Responses and snapshots from daemon to client
- **`PROTOCOL_VERSION`**: Handshake validation on connect

## Tiered refresh

All refresh intervals are configurable in the TOML config file.

| Data | Interval | Trigger |
|------|----------|---------|
| tmux panes + agent status | 2 seconds | Timer |
| Worktrees | 5 minutes | Timer |
| MR details | 60 seconds | Timer |
| MR list | 300 seconds | Timer + manual (`r` key) |

Codex hooks are an event-driven fast path layered on top of the tmux/agent polling interval. `UserPromptSubmit` marks the matching Codex pane Busy, `Stop` marks it Idle, and hook-derived status is prioritized over the SQLite polling heuristic for that Codex session. The regular polling path continues to refresh metadata from Codex's local SQLite databases.

## Paths

| Path | Purpose |
|------|---------|
| `/tmp/pertmux-{USER}.sock` | Daemon Unix socket |
| `/tmp/pertmux-daemon.log` | Daemon log file |
| `<data directory>/pertmux/read_state.db` | Comment read/unread tracking and persistent agent activity cards |
| `<data directory>/pertmux/last_project` | Last selected project persistence |

The data directory is `~/Library/Application Support` on macOS. On Linux it is
`$XDG_DATA_HOME`, which defaults to `~/.local/share`.
