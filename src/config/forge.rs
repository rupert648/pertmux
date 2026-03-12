use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectForge {
    Gitlab,
    Github,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitLabSourceConfig {
    #[serde(default = "default_gitlab_host")]
    pub host: String,
    pub token: Option<String>,
    // Backwards compat: old format stored project-level fields here
    pub project: Option<String>,
    pub local_path: Option<String>,
    pub username: Option<String>,
}

fn default_gitlab_host() -> String {
    "gitlab.com".to_string()
}

impl GitLabSourceConfig {
    pub fn api_token(&self) -> Option<String> {
        std::env::var("PERTMUX_GITLAB_TOKEN")
            .ok()
            .or_else(|| self.token.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubSourceConfig {
    #[serde(default = "default_github_host")]
    pub host: String,
    pub token: Option<String>,
}

fn default_github_host() -> String {
    "github.com".to_string()
}

impl GitHubSourceConfig {
    pub fn api_token(&self) -> Option<String> {
        std::env::var("PERTMUX_GITHUB_TOKEN")
            .ok()
            .or_else(|| self.token.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub source: ProjectForge,
    pub project: String,
    pub local_path: String,
    pub username: Option<String>,
}
