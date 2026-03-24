mod agent;
mod forge;
mod keybindings;

pub use agent::AgentConfig;
pub use forge::{GitHubSourceConfig, GitLabSourceConfig, ProjectConfig, ProjectForge};
pub use keybindings::KeybindingsConfig;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionConfig {
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub requires_mr: bool,
}

pub fn default_agent_actions() -> Vec<AgentActionConfig> {
    vec![
        AgentActionConfig {
            name: "Rebase with upstream".to_string(),
            prompt: "Rebase the current branch onto origin/{target_branch}. \
                     Pull the latest changes from origin/{target_branch} first, \
                     then rebase on top. Resolve any conflicts."
                .to_string(),
            requires_mr: false,
        },
        AgentActionConfig {
            name: "Check pipeline & fix errors".to_string(),
            prompt: "Check the CI/CD pipeline status for MR: {mr_url}\n\n\
                     Review any failing pipeline jobs. For each failure:\n\
                     1. Identify the root cause from the job logs\n\
                     2. Fix the issue in the code\n\
                     3. Commit the fix\n\n\
                     If there is no pipeline running, stop and report back."
                .to_string(),
            requires_mr: true,
        },
    ]
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub refresh_interval: u64,
    pub mr_detail_interval: u64,
    pub worktree_interval: u64,
    pub mr_list_interval: u64,
    pub default_agent_command: Option<String>,
    /// Command template for creating a worktree and immediately injecting a prompt.
    /// Use `{{msg}}` as the placeholder for the user-supplied message.
    /// Example: `"opencode run {{msg}}"` → user types a message → runs `opencode run <message>`.
    pub default_worktree_with_prompt: Option<String>,
    pub keybindings: KeybindingsConfig,
    pub agent: AgentConfig,
    pub gitlab: Option<GitLabSourceConfig>,
    pub github: Option<GitHubSourceConfig>,
    pub project: Option<Vec<ProjectConfig>>,
    #[serde(default = "default_agent_actions")]
    pub agent_action: Vec<AgentActionConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval: 2,
            mr_detail_interval: 60,
            worktree_interval: 30,
            mr_list_interval: 300,
            default_agent_command: None,
            default_worktree_with_prompt: None,
            keybindings: KeybindingsConfig::default(),
            agent: AgentConfig::default(),
            gitlab: None,
            github: None,
            project: None,
            agent_action: default_agent_actions(),
        }
    }
}

impl Config {
    pub fn resolve_projects(&self) -> Vec<ProjectConfig> {
        if let Some(ref projects) = self.project {
            return projects.clone();
        }

        if let Some(ref gl) = self.gitlab
            && let (Some(project), Some(local_path)) = (&gl.project, &gl.local_path)
        {
            let name = project
                .split('/')
                .next_back()
                .unwrap_or(project)
                .to_string();
            return vec![ProjectConfig {
                name,
                source: ProjectForge::Gitlab,
                project: project.clone(),
                local_path: local_path.clone(),
                username: gl.username.clone(),
            }];
        }

        vec![]
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let mut errors: Vec<String> = Vec::new();

        if self.project.is_some()
            && let Some(ref gl) = self.gitlab
            && gl.project.is_some()
        {
            errors.push(
                        "config: [gitlab] has 'project' field but [[project]] is also defined.\n\
                         hint: remove 'project' and 'local_path' from [gitlab] — use [[project]] instead."
                            .into(),
                    );
        }

        let projects = self.resolve_projects();

        for proj in &projects {
            if !std::path::Path::new(&proj.local_path).is_dir() {
                errors.push(format!(
                    "config: project '{}' local_path does not exist: {}\n\
                     hint: create the directory or fix the path in your config.",
                    proj.name, proj.local_path,
                ));
            }

            match proj.source {
                ProjectForge::Gitlab => {
                    if self.gitlab.is_none() {
                        errors.push(format!(
                            "config: project '{}' has source=\"gitlab\" but no [gitlab] section.\n\
                             hint: add a [gitlab] section with host and token.",
                            proj.name,
                        ));
                    }
                }
                ProjectForge::Github => {
                    if self.github.is_none() {
                        errors.push(format!(
                            "config: project '{}' has source=\"github\" but no [github] section.\n\
                             hint: add a [github] section with token.",
                            proj.name,
                        ));
                    }
                }
            }
        }

        if let Some(ref gl) = self.gitlab
            && gl.api_token().is_none()
        {
            errors.push(
                "config: [gitlab] has no token and PERTMUX_GITLAB_TOKEN is not set.\n\
                     hint: add token to [gitlab] or export PERTMUX_GITLAB_TOKEN."
                    .into(),
            );
        }

        if let Some(ref gh) = self.github
            && gh.api_token().is_none()
        {
            errors.push(
                "config: [github] has no token and PERTMUX_GITHUB_TOKEN is not set.\n\
                     hint: add token to [github] or export PERTMUX_GITHUB_TOKEN."
                    .into(),
            );
        }

        let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        for (i, name) in names.iter().enumerate() {
            if names[i + 1..].contains(name) {
                errors.push(format!("config: duplicate project name '{}'.", name));
            }
        }

        let kb = &self.keybindings;
        let mut key_map: HashMap<char, &str> = HashMap::new();
        let bindings = [
            (kb.refresh, "refresh"),
            (kb.open_browser, "open_browser"),
            (kb.copy_branch, "copy_branch"),
            (kb.filter_projects, "filter_projects"),
            (kb.create_worktree, "create_worktree"),
            (kb.delete_worktree, "delete_worktree"),
            (kb.merge_worktree, "merge_worktree"),
            (kb.agent_actions, "agent_actions"),
            (kb.mr_overview, "mr_overview"),
            (kb.activity_feed, "activity_feed"),
            (kb.open_worktree_with_prompt, "open_worktree_with_prompt"),
        ];
        for (ch, name) in &bindings {
            if let Some(existing) = key_map.get(ch) {
                errors.push(format!(
                    "config: duplicate keybinding '{}' — each action must have a unique key.\n\
                     hint: '{}' is used by both '{}' and '{}'.",
                    ch, ch, existing, name,
                ));
            } else {
                key_map.insert(*ch, name);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!("{}", errors.join("\n\n"))
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
            let xdg_path = dirs::home_dir().map(|h| h.join(".config").join("pertmux.toml"));
            let native_path = dirs::config_dir().map(|d| d.join("pertmux.toml"));

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
    fn test_old_format_backwards_compat() {
        let cfg = load_from_str(
            r#"
[gitlab]
token = "test-token"
host = "gitlab.example.com"
project = "team/project"
local_path = "/tmp/test-repo"
username = "alice"
"#,
        );
        let projects = cfg.resolve_projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].source, ProjectForge::Gitlab);
        assert_eq!(projects[0].project, "team/project");
        assert_eq!(projects[0].local_path, "/tmp/test-repo");
        assert_eq!(projects[0].username, Some("alice".to_string()));
        assert_eq!(projects[0].name, "project");
    }

    #[test]
    fn test_new_format_multi_project() {
        let cfg = load_from_str(
            r#"
[gitlab]
host = "gitlab.example.com"
token = "test-token"

[[project]]
name = "Alpha"
source = "gitlab"
project = "team/alpha"
local_path = "/tmp/alpha"

[[project]]
name = "Beta"
source = "gitlab"
project = "team/beta"
local_path = "/tmp/beta"
username = "bob"
"#,
        );
        let projects = cfg.resolve_projects();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "Alpha");
        assert_eq!(projects[0].project, "team/alpha");
        assert_eq!(projects[1].name, "Beta");
        assert_eq!(projects[1].username, Some("bob".to_string()));
    }

    #[test]
    fn test_new_format_ignores_gitlab_project_field() {
        let cfg = load_from_str(
            r#"
[gitlab]
host = "gitlab.example.com"
token = "test-token"

[[project]]
name = "Main"
source = "gitlab"
project = "team/main"
local_path = "/tmp/main"
"#,
        );
        let projects = cfg.resolve_projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Main");
    }

    #[test]
    fn test_no_gitlab_no_projects() {
        let cfg = load_from_str("refresh_interval = 2\n");
        assert!(cfg.gitlab.is_none());
        assert!(cfg.project.is_none());
        assert!(cfg.resolve_projects().is_empty());
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

    #[test]
    fn test_validate_missing_gitlab_section() {
        let cfg = load_from_str(
            r#"
[[project]]
name = "Test"
source = "gitlab"
project = "team/test"
local_path = "/tmp/test"
"#,
        );
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("no [gitlab] section"));
    }

    #[test]
    fn test_validate_missing_token() {
        if std::env::var("PERTMUX_GITLAB_TOKEN").is_ok() {
            return;
        }
        let cfg = load_from_str(
            r#"
[gitlab]
host = "gitlab.example.com"
project = "team/test"
local_path = "/tmp/test"
"#,
        );
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("no token"));
    }

    #[test]
    fn test_validate_ambiguous_old_and_new() {
        let cfg = load_from_str(
            r#"
[gitlab]
host = "gitlab.example.com"
token = "test-token"
project = "team/old"
local_path = "/tmp/old"

[[project]]
name = "New"
source = "gitlab"
project = "team/new"
local_path = "/tmp/new"
"#,
        );
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("[[project]] is also defined"));
    }

    #[test]
    fn test_validate_duplicate_project_names() {
        let cfg = load_from_str(
            r#"
[gitlab]
host = "gitlab.example.com"
token = "test-token"

[[project]]
name = "Same"
source = "gitlab"
project = "team/a"
local_path = "/tmp/a"

[[project]]
name = "Same"
source = "gitlab"
project = "team/b"
local_path = "/tmp/b"
"#,
        );
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("duplicate project name"));
    }

    #[test]
    fn test_validate_github_missing_section() {
        let cfg = load_from_str(
            r#"
[[project]]
name = "GH"
source = "github"
project = "org/repo"
local_path = "/tmp/gh"
"#,
        );
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("no [github] section"));
    }

    #[test]
    fn test_validate_github_missing_token() {
        if std::env::var("PERTMUX_GITHUB_TOKEN").is_ok() {
            return;
        }
        let cfg = load_from_str(
            r#"
[github]
host = "github.com"

[[project]]
name = "GH"
source = "github"
project = "org/repo"
local_path = "/tmp"
"#,
        );
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("PERTMUX_GITHUB_TOKEN"));
    }

    #[test]
    fn test_validate_github_passes() {
        let cfg = load_from_str(
            r#"
[github]
token = "ghp_test"

[[project]]
name = "GH"
source = "github"
project = "org/repo"
local_path = "/tmp"
"#,
        );
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_github_default_host() {
        let cfg = load_from_str(
            r#"
[github]
token = "ghp_test"
"#,
        );
        assert_eq!(cfg.github.unwrap().host, "github.com");
    }

    #[test]
    fn test_mixed_forge_projects() {
        let cfg = load_from_str(
            r#"
[gitlab]
host = "gitlab.example.com"
token = "gl-token"

[github]
token = "ghp-token"

[[project]]
name = "GL Project"
source = "gitlab"
project = "team/gl-app"
local_path = "/tmp/gl"

[[project]]
name = "GH Project"
source = "github"
project = "org/gh-app"
local_path = "/tmp/gh"
"#,
        );
        let projects = cfg.resolve_projects();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].source, ProjectForge::Gitlab);
        assert_eq!(projects[1].source, ProjectForge::Github);
    }

    #[test]
    fn test_validate_old_format_passes() {
        let cfg = load_from_str(
            r#"
[gitlab]
host = "gitlab.example.com"
token = "test-token"
project = "team/project"
local_path = "/tmp"
"#,
        );
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_new_format_passes() {
        let cfg = load_from_str(
            r#"
[gitlab]
host = "gitlab.example.com"
token = "test-token"

[[project]]
name = "Main"
source = "gitlab"
project = "team/main"
local_path = "/tmp"
"#,
        );
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_bad_local_path() {
        let cfg = load_from_str(
            r#"
[gitlab]
host = "gitlab.example.com"
token = "test-token"

[[project]]
name = "Bad"
source = "gitlab"
project = "team/bad"
local_path = "/nonexistent/path/here"
"#,
        );
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("local_path does not exist"));
    }

    #[test]
    fn test_unknown_source_rejected_at_parse() {
        let result: Result<Config, _> = toml::from_str(
            r#"
[[project]]
name = "Bad"
source = "bitbucket"
project = "team/bad"
local_path = "/tmp/bad"
"#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_keybindings_defaults() {
        let cfg = load_from_str("refresh_interval = 2\n");
        let kb = &cfg.keybindings;
        assert_eq!(kb.refresh, 'r');
        assert_eq!(kb.open_browser, 'o');
        assert_eq!(kb.copy_branch, 'b');
        assert_eq!(kb.filter_projects, 'f');
        assert_eq!(kb.create_worktree, 'c');
        assert_eq!(kb.delete_worktree, 'd');
        assert_eq!(kb.merge_worktree, 'M');
        assert_eq!(kb.mr_overview, 'm');
        assert_eq!(kb.activity_feed, 'A');
        assert_eq!(kb.open_worktree_with_prompt, 'w');
    }

    #[test]
    fn test_keybindings_custom() {
        let cfg = load_from_str(
            r#"
[keybindings]
refresh = "R"
open_browser = "O"
copy_branch = "y"
filter_projects = "p"
create_worktree = "n"
delete_worktree = "x"
merge_worktree = "g"
mr_overview = "v"
activity_feed = "G"
"#,
        );
        let kb = &cfg.keybindings;
        assert_eq!(kb.refresh, 'R');
        assert_eq!(kb.open_browser, 'O');
        assert_eq!(kb.copy_branch, 'y');
        assert_eq!(kb.filter_projects, 'p');
        assert_eq!(kb.create_worktree, 'n');
        assert_eq!(kb.delete_worktree, 'x');
        assert_eq!(kb.merge_worktree, 'g');
        assert_eq!(kb.mr_overview, 'v');
        assert_eq!(kb.activity_feed, 'G');
    }

    #[test]
    fn test_keybindings_partial_override() {
        let cfg = load_from_str(
            r#"
[keybindings]
refresh = "R"
"#,
        );
        assert_eq!(cfg.keybindings.refresh, 'R');
        assert_eq!(cfg.keybindings.open_browser, 'o');
        assert_eq!(cfg.keybindings.copy_branch, 'b');
    }

    #[test]
    fn test_keybindings_mr_overview_default() {
        let cfg = load_from_str("");
        assert_eq!(cfg.keybindings.mr_overview, 'm');
    }

    #[test]
    fn test_keybindings_duplicate_rejected() {
        let cfg = load_from_str(
            r#"
[keybindings]
refresh = "r"
open_browser = "r"
"#,
        );
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("duplicate keybinding 'r'"));
    }

    #[test]
    fn test_keybindings_missing_section_uses_defaults() {
        let cfg = load_from_str(
            r#"
[gitlab]
host = "gitlab.example.com"
token = "test-token"
project = "team/project"
local_path = "/tmp"
"#,
        );
        assert_eq!(cfg.keybindings.refresh, 'r');
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_keybindings_default_validation_passes() {
        let cfg = load_from_str("refresh_interval = 2\n");
        assert!(cfg.validate().is_ok());
    }
}
