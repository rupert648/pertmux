use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct GitLabConfig {
    pub token: Option<String>,
    #[serde(default = "default_gitlab_host")]
    pub host: String,
    pub project: String,
    pub local_path: String,
    pub username: Option<String>,
}

fn default_gitlab_host() -> String {
    "gitlab.com".to_string()
}

impl GitLabConfig {
    /// Resolve API token: PERTMUX_GITLAB_TOKEN env var takes priority over config token.
    pub fn api_token(&self) -> Option<String> {
        std::env::var("PERTMUX_GITLAB_TOKEN")
            .ok()
            .or_else(|| self.token.clone())
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub refresh_interval: u64,
    pub agent: AgentConfig,
    pub gitlab: Option<GitLabConfig>,
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
            gitlab: None,
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
            let xdg_path =
                dirs::home_dir().map(|h| h.join(".config").join("pertmux").join("pertmux.toml"));
            let native_path = dirs::config_dir().map(|d| d.join("pertmux").join("pertmux.toml"));

            let found = xdg_path
                .filter(|p| p.exists())
                .or_else(|| native_path.filter(|p| p.exists()));

            match found {
                Some(p) => p,
                None => return Ok(Config::default()),
            }
        }
    };

    let content = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_from_str(s: &str) -> Config {
        toml::from_str(s).expect("parse failed")
    }

    #[test]
    fn test_gitlab_config_present() {
        let cfg = load_from_str(
            r#"
[gitlab]
token = "test-token"
host = "gitlab.example.com"
project = "team/project"
local_path = "/tmp/test-repo"
"#,
        );
        let gl = cfg.gitlab.expect("gitlab should be Some");
        assert_eq!(gl.token, Some("test-token".to_string()));
        assert_eq!(gl.host, "gitlab.example.com");
        assert_eq!(gl.project, "team/project");
        assert_eq!(gl.local_path, "/tmp/test-repo");
    }

    #[test]
    fn test_gitlab_config_absent() {
        let cfg = load_from_str("refresh_interval = 2\n");
        assert!(cfg.gitlab.is_none());
    }

    #[test]
    fn test_gitlab_default_host() {
        let cfg = load_from_str(
            r#"
[gitlab]
project = "team/project"
local_path = "/tmp/test-repo"
"#,
        );
        assert_eq!(cfg.gitlab.unwrap().host, "gitlab.com");
    }
}
