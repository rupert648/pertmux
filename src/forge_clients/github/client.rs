use crate::forge_clients::github::types::*;
use crate::forge_clients::traits::ForgeClient;
use crate::forge_clients::types::*;
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;

pub struct GitHubClient {
    client: Client,
    base_url: String,
    token: String,
    owner: String,
    repo: String,
    username: Option<String>,
}

impl GitHubClient {
    pub fn new(token: String, host: &str, project: &str, username: Option<String>) -> Self {
        let base_url = if host == "github.com" {
            "https://api.github.com".to_string()
        } else {
            format!("https://{}/api/v3", host)
        };

        let (owner, repo) = project.split_once('/').unwrap_or(("", project));

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("pertmux")
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url,
            token,
            owner: owner.to_string(),
            repo: repo.to_string(),
            username,
        }
    }

    fn gh_pr_to_summary(&self, pr: &GhPullRequest) -> MergeRequestSummary {
        MergeRequestSummary {
            iid: pr.number,
            title: pr.title.clone(),
            state: normalize_state(&pr.state),
            source_branch: pr.head.branch.clone(),
            target_branch: pr.base.branch.clone(),
            author: ForgeUser {
                id: pr.user.id,
                username: pr.user.login.clone(),
                name: pr
                    .user
                    .name
                    .clone()
                    .unwrap_or_else(|| pr.user.login.clone()),
            },
            draft: pr.draft,
            user_notes_count: pr.comments + pr.review_comments,
            web_url: pr.html_url.clone(),
            created_at: pr.created_at.clone(),
            updated_at: pr.updated_at.clone(),
            detailed_merge_status: pr.mergeable_state.as_deref().map(map_mergeable_state),
            has_conflicts: pr
                .mergeable
                .and_then(|m| {
                    if !m && pr.mergeable_state.as_deref() == Some("dirty") {
                        Some(true)
                    } else {
                        Some(false)
                    }
                }),
        }
    }

    fn gh_pr_to_detail(&self, pr: &GhPullRequest) -> MergeRequestDetail {
        MergeRequestDetail {
            iid: pr.number,
            title: pr.title.clone(),
            state: normalize_state(&pr.state),
            source_branch: pr.head.branch.clone(),
            target_branch: pr.base.branch.clone(),
            author: ForgeUser {
                id: pr.user.id,
                username: pr.user.login.clone(),
                name: pr
                    .user
                    .name
                    .clone()
                    .unwrap_or_else(|| pr.user.login.clone()),
            },
            draft: pr.draft,
            user_notes_count: pr.comments + pr.review_comments,
            web_url: pr.html_url.clone(),
            created_at: pr.created_at.clone(),
            updated_at: pr.updated_at.clone(),
            detailed_merge_status: pr.mergeable_state.as_deref().map(map_mergeable_state),
            has_conflicts: pr
                .mergeable
                .and_then(|m| {
                    if !m && pr.mergeable_state.as_deref() == Some("dirty") {
                        Some(true)
                    } else {
                        Some(false)
                    }
                }),
            assignees: vec![],
            reviewers: vec![],
            head_pipeline: None,
            head_sha: Some(pr.head.sha.clone()),
        }
    }
}

fn normalize_state(state: &str) -> String {
    match state {
        "open" => "opened".to_string(),
        other => other.to_string(),
    }
}

fn map_mergeable_state(state: &str) -> String {
    match state {
        "clean" => "mergeable".to_string(),
        "dirty" => "conflicts".to_string(),
        "blocked" => "blocked_status".to_string(),
        "behind" => "need_rebase".to_string(),
        "unstable" => "ci_must_pass".to_string(),
        "draft" => "draft_status".to_string(),
        "unknown" => "unknown".to_string(),
        other => other.to_string(),
    }
}

fn map_check_run_status(status: &str, conclusion: Option<&str>) -> String {
    match (status, conclusion) {
        ("completed", Some("success")) => "success".to_string(),
        ("completed", Some("failure")) => "failed".to_string(),
        ("completed", Some("cancelled")) => "canceled".to_string(),
        ("completed", Some("skipped")) => "skipped".to_string(),
        ("completed", Some("timed_out")) => "failed".to_string(),
        ("completed", Some("action_required")) => "manual".to_string(),
        ("completed", Some("neutral")) => "success".to_string(),
        ("completed", _) => "success".to_string(),
        ("in_progress", _) => "running".to_string(),
        ("queued", _) => "pending".to_string(),
        ("waiting", _) => "waiting_for_resource".to_string(),
        _ => "created".to_string(),
    }
}

#[async_trait(?Send)]
impl ForgeClient for GitHubClient {
    async fn fetch_mrs(&self) -> Result<Vec<MergeRequestSummary>> {
        let url = format!(
            "{}/repos/{}/{}/pulls?state=open&per_page=100",
            self.base_url, self.owner, self.repo
        );
        let prs: Vec<GhPullRequest> = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .context(format!("Failed to fetch PR list from {}", url))?
            .error_for_status()
            .context("GitHub API returned error status for PR list")?
            .json()
            .await
            .context("Failed to parse PR list response")?;

        let mut summaries: Vec<MergeRequestSummary> = prs
            .iter()
            .map(|pr| self.gh_pr_to_summary(pr))
            .collect();

        if let Some(ref username) = self.username {
            summaries.retain(|mr| mr.author.username == *username);
        }

        Ok(summaries)
    }

    async fn fetch_mr_detail(&self, iid: u64) -> Result<MergeRequestDetail> {
        let url = format!(
            "{}/repos/{}/{}/pulls/{}",
            self.base_url, self.owner, self.repo, iid
        );
        let pr: GhPullRequest = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .context(format!("Failed to fetch PR detail from {}", url))?
            .error_for_status()
            .context(format!(
                "GitHub API returned error status for PR {}",
                iid
            ))?
            .json()
            .await
            .context(format!("Failed to parse PR detail response for {}", iid))?;

        Ok(self.gh_pr_to_detail(&pr))
    }

    async fn fetch_ci_jobs(
        &self,
        mr_detail: &MergeRequestDetail,
    ) -> Result<Vec<PipelineJob>> {
        let sha = match &mr_detail.head_sha {
            Some(sha) => sha.clone(),
            None => return Ok(vec![]),
        };

        let url = format!(
            "{}/repos/{}/{}/commits/{}/check-runs?per_page=100",
            self.base_url, self.owner, self.repo, sha
        );
        let resp: GhCheckRunsResponse = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .context(format!("Failed to fetch check runs from {}", url))?
            .error_for_status()
            .context("GitHub API returned error for check runs")?
            .json()
            .await
            .context("Failed to parse check runs response")?;

        let jobs = resp
            .check_runs
            .iter()
            .map(|cr| PipelineJob {
                id: cr.id,
                name: cr.name.clone(),
                stage: cr
                    .app
                    .as_ref()
                    .and_then(|a| a.slug.clone())
                    .unwrap_or_else(|| "checks".to_string()),
                status: map_check_run_status(&cr.status, cr.conclusion.as_deref()),
                duration: None,
                allow_failure: false,
            })
            .collect();

        Ok(jobs)
    }

    async fn fetch_notes(&self, iid: u64) -> Result<Vec<MergeRequestNote>> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/comments?per_page=100",
            self.base_url, self.owner, self.repo, iid
        );
        let comments: Vec<GhIssueComment> = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .context(format!("Failed to fetch issue comments from {}", url))?
            .error_for_status()
            .context(format!(
                "GitHub API returned error status for PR {} comments",
                iid
            ))?
            .json()
            .await
            .context(format!(
                "Failed to parse comments response for PR {}",
                iid
            ))?;

        let notes = comments
            .into_iter()
            .map(|c| MergeRequestNote {
                id: c.id,
                body: c.body.unwrap_or_default(),
                author: ForgeUser {
                    id: c.user.id,
                    username: c.user.login.clone(),
                    name: c.user.name.unwrap_or_else(|| c.user.login),
                },
                created_at: c.created_at,
                system: false,
            })
            .collect();

        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_state() {
        assert_eq!(normalize_state("open"), "opened");
        assert_eq!(normalize_state("closed"), "closed");
        assert_eq!(normalize_state("merged"), "merged");
    }

    #[test]
    fn test_map_mergeable_state() {
        assert_eq!(map_mergeable_state("clean"), "mergeable");
        assert_eq!(map_mergeable_state("dirty"), "conflicts");
        assert_eq!(map_mergeable_state("blocked"), "blocked_status");
        assert_eq!(map_mergeable_state("behind"), "need_rebase");
        assert_eq!(map_mergeable_state("unstable"), "ci_must_pass");
        assert_eq!(map_mergeable_state("draft"), "draft_status");
        assert_eq!(map_mergeable_state("unknown"), "unknown");
        assert_eq!(map_mergeable_state("something_else"), "something_else");
    }

    #[test]
    fn test_map_check_run_status() {
        assert_eq!(
            map_check_run_status("completed", Some("success")),
            "success"
        );
        assert_eq!(
            map_check_run_status("completed", Some("failure")),
            "failed"
        );
        assert_eq!(
            map_check_run_status("completed", Some("cancelled")),
            "canceled"
        );
        assert_eq!(map_check_run_status("in_progress", None), "running");
        assert_eq!(map_check_run_status("queued", None), "pending");
        assert_eq!(
            map_check_run_status("waiting", None),
            "waiting_for_resource"
        );
    }

    #[test]
    fn test_github_client_new_public() {
        let client = GitHubClient::new(
            "ghp_test".to_string(),
            "github.com",
            "rupert648/pertmux",
            None,
        );
        assert_eq!(client.base_url, "https://api.github.com");
        assert_eq!(client.owner, "rupert648");
        assert_eq!(client.repo, "pertmux");
    }

    #[test]
    fn test_github_client_new_enterprise() {
        let client = GitHubClient::new(
            "ghp_test".to_string(),
            "github.corp.com",
            "team/app",
            Some("alice".to_string()),
        );
        assert_eq!(client.base_url, "https://github.corp.com/api/v3");
        assert_eq!(client.owner, "team");
        assert_eq!(client.repo, "app");
        assert_eq!(client.username, Some("alice".to_string()));
    }
}
