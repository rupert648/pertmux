use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub opencode: Option<OpenCodeAgentConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct OpenCodeAgentConfig {
    pub db_path: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            opencode: Some(OpenCodeAgentConfig::default()),
        }
    }
}
