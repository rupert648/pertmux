use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Refresh interval in seconds.
    pub refresh_interval: u64,

    /// Database settings.
    pub opencode: OpencodeConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct OpencodeConfig {
    /// Override path to the opencode SQLite database.
    /// Defaults to `~/.local/share/opencode/opencode.db`.
    pub path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval: 2,
            opencode: OpencodeConfig::default(),
        }
    }
}

impl Default for OpencodeConfig {
    fn default() -> Self {
        Self { path: None }
    }
}

/// Load configuration from a TOML file.
///
/// - If `explicit_path` is provided (via `-c`), it MUST exist or we return an error.
/// - Otherwise, try `~/.config/pertmux/pertmux.toml`. If it doesn't exist, return defaults.
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
