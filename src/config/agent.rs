use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub opencode: Option<OpenCodeAgentConfig>,
    pub claude_code: Option<ClaudeCodeAgentConfig>,
    pub codex: Option<CodexAgentConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct OpenCodeAgentConfig {
    pub db_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ClaudeCodeAgentConfig {}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CodexAgentConfig {
    /// Override the Codex home directory (default: `~/.codex`).
    pub codex_home: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            opencode: Some(OpenCodeAgentConfig::default()),
            claude_code: None,
            codex: None,
        }
    }
}
