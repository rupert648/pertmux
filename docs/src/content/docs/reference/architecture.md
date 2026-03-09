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

| Data | Interval | Trigger |
|------|----------|---------|
| tmux panes + agent status | 2 seconds | Timer |
| Worktrees | 30 seconds | Timer |
| MR details | 60 seconds | Timer |
| MR list | Manual | `r` key or daemon startup |

## Paths

| Path | Purpose |
|------|---------|
| `/tmp/pertmux-{USER}.sock` | Daemon Unix socket |
| `/tmp/pertmux-daemon.log` | Daemon log file |
| `~/.local/share/pertmux/read_state.db` | Comment read/unread tracking |
