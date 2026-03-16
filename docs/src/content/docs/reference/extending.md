---
title: Extending pertmux
description: Add custom coding agents and forge integrations.
---

pertmux is designed to be extensible. You can add new coding agents and forge integrations by implementing the appropriate traits.

## Adding a coding agent

Implement the `CodingAgent` trait in `src/coding_agent/`:

```rust
pub trait CodingAgent {
    fn name(&self) -> &str;
    fn process_name(&self) -> &str;
    fn query_status(&self, pane_pid: u32) -> PaneStatus;
    fn send_prompt(
        &self,
        pane_pid: u32,
        session_id: &str,
        prompt: &str,
    ) -> anyhow::Result<String>;
}
```

- **`name()`**: Human-readable name for the agent.
- **`process_name()`**: The process name to detect in tmux panes.
- **`query_status()`**: Takes the pane's PID and returns a `PaneStatus` enum (Busy, Idle, Retry, Unknown).
- **`send_prompt()`**: Delivers a prompt to the agent. The implementation determines the delivery mechanism — for example, opencode uses its HTTP API (`POST /session/{id}/message`), but another agent might use tmux `send-keys` or a Unix socket.

Database enrichment (e.g., fetching session details, token usage, and message history) happens separately in `db::enrich_pane()`.

Register your agent in `agents_from_config()` in `src/coding_agent/mod.rs`.

## Adding a forge

Implement the `ForgeClient` trait in `src/forge_clients/`:

```rust
#[async_trait(?Send)]
pub trait ForgeClient {
    async fn fetch_mrs(&self) -> anyhow::Result<Vec<MergeRequestSummary>>;
    async fn fetch_mr_detail(&self, iid: u64) -> anyhow::Result<MergeRequestDetail>;
    async fn fetch_ci_jobs(&self, mr: &MergeRequestDetail) -> anyhow::Result<Vec<PipelineJob>>;
    async fn fetch_notes(&self, iid: u64) -> anyhow::Result<Vec<MergeRequestNote>>;
}
```

Each forge handles its own:
- API authentication
- Response parsing
- State normalization (e.g., GitHub `"open"` becomes `"opened"`)
