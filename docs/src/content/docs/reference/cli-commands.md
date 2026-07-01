---
title: CLI Commands
description: All pertmux command-line commands and options.
---

## Commands

### `pertmux serve`

Start the background daemon.

```bash
pertmux serve                    # backgrounds automatically
pertmux -c config.toml serve     # with specific config
pertmux serve --foreground       # stay in terminal (for debugging)
```

The daemon forks to the background by default, logging to `/tmp/pertmux-daemon.log`. It validates your config and checks for an existing daemon before forking — errors show immediately in your terminal. Use `--foreground` to keep the daemon in your terminal for debugging.

The daemon runs until stopped with `pertmux stop`.

### `pertmux connect`

Open the TUI client and connect to the running daemon.

```bash
pertmux connect
```

Fails with a clear error if the daemon is not running.

### `pertmux stop`

Stop the running daemon.

```bash
pertmux stop
```

### `pertmux status`

Show the daemon socket path and whether it's running.

```bash
pertmux status
```

### `pertmux cleanup`

Clean up stale files and persistence data.

```bash
pertmux cleanup
```

- Removes the stale socket file if the daemon is not running.
- Removes `read_state.db` (comment tracking) and `last_project` persistence files.
- Skips the live socket if the daemon is still running.

### `pertmux install`

Install optional integrations.

```bash
pertmux install --codex-hooks
pertmux install --codex-hooks --local
pertmux install --codex-hooks --local --repo /path/to/repo
pertmux install --codex-hooks --force
```

`--codex-hooks` installs global Codex hooks into `~/.codex/hooks.json` by default. The hooks call `pertmux codex-hook` for `SessionStart`, `UserPromptSubmit`, and `Stop`, allowing Codex to notify the daemon immediately when a session starts, a user prompt is submitted, or a turn finishes.

Codex requires non-managed command hooks to be reviewed and trusted. After installing, start Codex and run `/hooks` once to trust the generated hook definitions. For one-off testing, you can start Codex with `--dangerously-bypass-hook-trust`.

| Option | Description |
|--------|-------------|
| `--codex-hooks` | Install the Codex hook integration |
| `--local` | Install repo-local hooks into `.codex/hooks.json` instead of global hooks |
| `--repo <path>` | Repository path for `--local` installs; defaults to the current directory |
| `--force` | Replace an invalid existing hooks file |

## Global options

| Option | Description |
|--------|-------------|
| `-c`, `--config <path>` | Path to TOML config file |
| `--version` | Show version |
| `-h`, `--help` | Show help |
