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
    fn process_names(&self) -> &[&str];
    fn enrich_pane(&self, pane: &mut AgentPane);
}
```

- **`name()`**: Human-readable name for the agent
- **`process_names()`**: Process names to detect in tmux panes
- **`enrich_pane()`**: Query agent-specific data and populate the `AgentPane` struct

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
