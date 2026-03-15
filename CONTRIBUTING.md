# Contributing to pertmux

Contributions are welcome! For anything beyond a small bug fix, please open an issue first so we can discuss the approach before you invest time in a PR.

## Development setup

```sh
git clone https://github.com/rupert648/pertmux.git
cd pertmux
cargo build
```

**Runtime requirement**: pertmux must run inside a tmux session. For development, use `pertmux serve --foreground` to keep the daemon in the terminal with visible logs.

## Running CI checks locally

Run these before pushing — they match what CI runs:

```sh
cargo fmt --all --check          # formatting (run `cargo fmt --all` to fix)
cargo clippy --all-targets --all-features -- -D warnings   # lints
cargo test --all-features        # tests
```

The docs site has its own checks:

```sh
cd docs
npm ci
npx astro check
npm run build
```

## Code style

- **Formatting**: `cargo fmt --all` — standard stable rustfmt, no custom config
- **Lints**: clippy warnings are errors in CI (`-D warnings`). Fix them rather than suppressing with `#[allow(...)]`
- **No unsafe**: pertmux has no unsafe code and should stay that way
- **No type suppression**: avoid `as any`, `@ts-ignore`, or equivalent escape hatches
- **Edition**: Rust 2024
- **Toolchain**: current stable (no nightly required)

## Architecture

See [AGENTS.md](AGENTS.md) for a full technical overview of the codebase, module guide, and design decisions.

pertmux is designed to be extensible via two key traits:

- **`CodingAgent`** (`src/coding_agent/mod.rs`) — add support for new coding agents
- **`ForgeClient`** (`src/forge_clients/traits.rs`) — add support for new forges (e.g. Bitbucket, Gitea)

See the [Extending pertmux](https://pertmux.dev/reference/extending/) docs for implementation details.

## Testing

- Run `cargo test --all-features`
- If you fix a bug, add a regression test
- Unit tests don't require tmux — full integration testing does

## Pull requests

- Keep PRs focused — one logical change per PR
- Write a clear description of what changed and why
- Link to the issue it addresses (`Fixes #123`)
- PRs are squash-merged

## Reporting issues

Please include:

- pertmux version (`pertmux --version`)
- OS and architecture
- tmux version (`tmux -V`)
- Relevant config (redact tokens)
- Steps to reproduce

## License

By contributing, you agree that your contributions will be licensed under the same [MIT License](LICENSE) that covers the project.
