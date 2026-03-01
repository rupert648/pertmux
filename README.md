# pertmux

pertmux ([ru]-pert multiplexer) is a highly personally and opinionated rust tui dashboard
for monitoring opencode (https://github.com/sst/opencode) instances running inside tmux.
It auto discovers every opencode instance across all tmux panes, queries their state via HTTP API +
SQLite database, and renders a live dashboard with session details.

## Setup

### Prerequisites
* [opencode](https://github.com/sst/opencode)

You should configure opencode to start its server alongside so that pertmux can query status.
The easiest way to do this is by aliasing the opencode command:
```
alias opencode='command opencode --port 0'
```
`--port 0` tells opencode to use a random port. This allows you to have multiple opencode sessions and pertmux
does the hard work of finding their pids & ports.


### Install: 

TODO

## Configuration

pertmux works out of the box with zero configuration. All settings have sensible defaults.

To customize behavior, create a TOML config file:

```
pertmux -c ./path/to/config.toml
```

If no `-c` flag is provided, pertmux looks for `~/.config/pertmux/pertmux.toml`. If that file doesn't exist, defaults are used.

### Config file format

```toml
# Refresh interval in seconds (default: 2)
refresh_interval = 2

[opencode]
# Override path to the opencode SQLite database
# Default: ~/.local/share/opencode/opencode.db
# path = "/path/to/opencode.db"
```

### Options

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `refresh_interval` | integer | `2` | How often (in seconds) to poll tmux panes and refresh the dashboard |

#### `[opencode]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `path` | string | `~/.local/share/opencode/opencode.db` | Path to the opencode SQLite database. Only needed if your database is in a non-standard location |
