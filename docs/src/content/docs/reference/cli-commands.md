---
title: CLI Commands
description: All pertmux command-line commands and options.
---

## Commands

### `pertmux serve`

Start the background daemon.

```bash
pertmux serve
pertmux -c config.toml serve
```

The daemon must be started before any client can connect. It runs until killed or stopped with `pertmux stop`.

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

## Global options

| Option | Description |
|--------|-------------|
| `-c`, `--config <path>` | Path to TOML config file |
| `--version` | Show version |
| `-h`, `--help` | Show help |
