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
    fn query_status(&self, pane: &AgentPane) -> PaneStatus;
    fn send_prompt(&self, pane_pid: u32, session_id: &str, prompt: &str) -> anyhow::Result<String>;
    fn enrich_pane(&self, _pane: &mut AgentPane) {}
    fn fetch_session_detail(&self, _session_id: &str) -> Option<SessionDetail> { None }
}
```

- **`name()`**: Human-readable name for the agent (e.g. `"opencode"`, `"claude-code"`).
- **`process_name()`**: The process name to detect in tmux panes (e.g. `"opencode"`, `"claude"`).
- **`query_status()`**: Takes the pane info and returns a `PaneStatus` enum (Busy, Idle, Retry, Unknown).
- **`send_prompt()`**: Delivers a prompt to the agent. The implementation determines the delivery mechanism — opencode uses its HTTP API, Claude Code uses tmux `send-keys`.
- **`enrich_pane()`**: Populates pane metadata (session title, model, tokens, etc.) from the agent's data source.
- **`fetch_session_detail()`**: Returns detailed session info for the detail panel.

Each agent is responsible for its own data source — opencode uses HTTP API + SQLite, Claude Code reads JSONL transcript files.

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
