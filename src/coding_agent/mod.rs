pub mod opencode;

use crate::types::PaneStatus;

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
    /// Given the PID of the tmux pane's shell process, discover the agent's
    /// communication channel and retrieve its current status.
    fn query_status(&self, pane_pid: u32) -> PaneStatus;

    /// Send a prompt to the coding agent.
    ///
    /// Given the PID of the tmux pane's shell process and a session identifier,
    /// deliver the prompt text to the agent. The agent implementation determines
    /// the delivery mechanism (e.g. HTTP API, tmux send-keys, socket).
    fn send_prompt(&self, pane_pid: u32, session_id: &str, prompt: &str) -> anyhow::Result<String>;
}

pub fn agents_from_config(config: &crate::config::AgentConfig) -> Vec<Box<dyn CodingAgent>> {
    let mut agents: Vec<Box<dyn CodingAgent>> = Vec::new();
    if config.opencode.is_some() {
        agents.push(Box::new(opencode::OpenCode));
    }
    agents
}
