---
title: Installation
description: How to install pertmux on your system.
---

## Prerequisites

- [tmux](https://github.com/tmux/tmux) 3.2+ (for popup support)
- [Rust toolchain](https://rustup.rs/) (for building from source)
- [worktrunk](https://github.com/max-sixty/worktrunk) (optional) — enables the worktree management panel

## Install from source

```bash
cargo install --path .
```

Or build manually:

```bash
cargo build --release
# Binary at target/release/pertmux
```

## Install worktrunk (optional)

Worktrunk powers the worktree management panel. Install it and ensure `wt` is on your PATH:

```bash
cargo install worktrunk
```

## Verify installation

```bash
pertmux --version
```
