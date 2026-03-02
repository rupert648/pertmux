use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub refresh_interval: u64,
    pub agent: AgentConfig,
}

/// Agent configuration. Each field corresponds to a coding agent.
/// `Some(...)` = enabled, `None` = disabled.
/// When omitted entirely from the config file, all agents are enabled with defaults.
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

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval: 2,
            agent: AgentConfig::default(),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            opencode: Some(OpenCodeAgentConfig::default()),
        }
    }
}


pub fn load(explicit_path: Option<&str>) -> anyhow::Result<Config> {
    let path = match explicit_path {
        Some(p) => {
            let p = PathBuf::from(p);
            if !p.exists() {
                anyhow::bail!("config file not found: {}", p.display());
            }
            p
        }
        None => {
            let default_path = dirs::config_dir()
                .map(|d| d.join("pertmux").join("pertmux.toml"))
                .unwrap_or_else(|| PathBuf::from("pertmux.toml"));

            if !default_path.exists() {
                return Ok(Config::default());
            }
            default_path
        }
    };

    let content = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
