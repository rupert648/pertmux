pub mod claude_code;
pub mod opencode;

use crate::types::{AgentPane, PaneStatus, SessionDetail};
use sysinfo::System;

/// Trait for coding agent integrations.
///
/// Each implementation knows how to detect its process in tmux panes,
/// query session status, and send prompts through its own mechanism
/// (HTTP API, socket, file, etc.).
///
/// To add a new agent, implement this trait and register it in [`default_agents`].
#[allow(dead_code)]
pub trait CodingAgent {
    /// Display name for the UI.
    fn name(&self) -> &str;

    /// Process name to match against tmux `pane_current_command`.
    fn process_name(&self) -> &str;

    /// Query the live status of a coding session.
    ///
    /// Accepts a pre-refreshed `&System` for agents that need process-tree
    /// inspection (e.g. opencode port discovery). Agents that don't need it
    /// can ignore the parameter.
    fn query_status(&self, pane: &AgentPane, sys: &System) -> PaneStatus;

    /// Send a prompt to the coding agent.
    ///
    /// Given the PID of the tmux pane's shell process and a session identifier,
    /// deliver the prompt text to the agent. The agent implementation determines
    /// the delivery mechanism (e.g. HTTP API, tmux send-keys, socket).
    fn send_prompt(&self, pane_pid: u32, session_id: &str, prompt: &str) -> anyhow::Result<String>;

    fn enrich_pane(&self, _pane: &mut AgentPane) {}

    fn fetch_session_detail(&self, _session_id: &str) -> Option<SessionDetail> {
        None
    }
}

pub fn agents_from_config(config: &crate::config::AgentConfig) -> Vec<Box<dyn CodingAgent>> {
    let mut agents: Vec<Box<dyn CodingAgent>> = Vec::new();
    if config.opencode.is_some() {
        agents.push(Box::new(opencode::OpenCode::new(
            config.opencode.as_ref().and_then(|c| c.db_path.clone()),
        )));
    }
    if config.claude_code.is_some() {
        agents.push(Box::new(claude_code::ClaudeCode));
    }
    agents
}
