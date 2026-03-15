---
title: Installation
description: How to install pertmux on your system.
---

## Prerequisites

- [tmux](https://github.com/tmux/tmux) 3.2+ (for popup support)
- [Rust toolchain](https://rustup.rs/)
- [worktrunk](https://github.com/max-sixty/worktrunk) (optional) — enables the worktree management panel

## Install from crates.io

```bash
cargo install pertmux
```

## Install from source

```bash
git clone https://github.com/rupert648/pertmux.git
cd pertmux
cargo install --path .
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
