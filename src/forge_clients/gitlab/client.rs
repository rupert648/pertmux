use crate::forge_clients::gitlab::types::GlDiscussion;
use crate::forge_clients::traits::ForgeClient;
use crate::forge_clients::types::*;
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GitLabUserMr {
    iid: u64,
    title: String,
    web_url: String,
    draft: bool,
    updated_at: jiff::Timestamp,
    author: crate::forge_clients::types::ForgeUser,
}

fn extract_gitlab_project_path(web_url: &str) -> Option<String> {
    let path_end = web_url.find("/-/")?;
    let prefix = &web_url[..path_end];
    let path_without_host = prefix.split('/').skip(3).collect::<Vec<_>>().join("/");
    if path_without_host.is_empty() {
        None
    } else {
        Some(path_without_host)
    }
}

pub struct GitLabClient {
    client: Client,
    base_url: String,
    token: String,
    project_id: String,
    username: Option<String>,
}

impl GitLabClient {
    pub fn new(token: String, host: &str, project: &str, username: Option<String>) -> Self {
        let base_url = format!("https://{}/api/v4", host);
        let project_id = project.replace('/', "%2F");
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url,
            token,
            project_id,
            username,
        }
    }

    async fn fetch_pipeline_jobs(&self, pipeline_id: u64) -> Result<Vec<PipelineJob>> {
        let url = format!(
            "{}/projects/{}/pipelines/{}/jobs?per_page=100",
            self.base_url, self.project_id, pipeline_id
        );
        self.client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .context(format!("Failed to fetch pipeline jobs from {}", url))?
            .error_for_status()
            .context(format!(
                "GitLab API returned error for pipeline {} jobs",
                pipeline_id
            ))?
            .json::<Vec<PipelineJob>>()
            .await
            .context(format!(
                "Failed to parse pipeline jobs for pipeline {}",
                pipeline_id
            ))
    }
}

#[async_trait(?Send)]
impl ForgeClient for GitLabClient {
    async fn fetch_mrs(&self) -> Result<Vec<MergeRequestSummary>> {
        let author_filter = self
            .username
            .as_deref()
            .map(|u| format!("&author_username={}", u))
            .unwrap_or_default();
        let url = format!(
            "{}/projects/{}/merge_requests?state=opened&per_page=100{}",
            self.base_url, self.project_id, author_filter
        );
        self.client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .context(format!("Failed to fetch MR list from {}", url))?
            .error_for_status()
            .context("GitLab API returned error status for MR list")?
            .json::<Vec<MergeRequestSummary>>()
            .await
            .context("Failed to parse MR list response")
    }

    async fn fetch_mr_detail(&self, iid: u64) -> Result<MergeRequestDetail> {
        let url = format!(
            "{}/projects/{}/merge_requests/{}",
            self.base_url, self.project_id, iid
        );
        self.client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .context(format!("Failed to fetch MR detail from {}", url))?
            .error_for_status()
            .context(format!("GitLab API returned error status for MR {}", iid))?
            .json::<MergeRequestDetail>()
            .await
            .context(format!("Failed to parse MR detail response for {}", iid))
    }

    async fn fetch_ci_jobs(&self, mr_detail: &MergeRequestDetail) -> Result<Vec<PipelineJob>> {
        let pipeline_id = mr_detail.head_pipeline.as_ref().map(|p| p.id);

        match pipeline_id {
            Some(pid) => self.fetch_pipeline_jobs(pid).await,
            None => Ok(vec![]),
        }
    }

    async fn fetch_notes(&self, iid: u64) -> Result<Vec<MergeRequestNote>> {
        let url = format!(
            "{}/projects/{}/merge_requests/{}/notes?per_page=100",
            self.base_url, self.project_id, iid
        );
        self.client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .context(format!("Failed to fetch MR notes from {}", url))?
            .error_for_status()
            .context(format!(
                "GitLab API returned error status for MR {} notes",
                iid
            ))?
            .json::<Vec<MergeRequestNote>>()
            .await
            .context(format!("Failed to parse MR notes response for {}", iid))
    }

    async fn fetch_discussions(&self, iid: u64) -> Result<Vec<MergeRequestThread>> {
        let url = format!(
            "{}/projects/{}/merge_requests/{}/discussions?per_page=100",
            self.base_url, self.project_id, iid
        );
        let discussions: Vec<GlDiscussion> = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .context(format!("Failed to fetch MR discussions from {}", url))?
            .error_for_status()
            .context(format!(
                "GitLab API returned error status for MR {} discussions",
                iid
            ))?
            .json()
            .await
            .context(format!(
                "Failed to parse MR discussions response for {}",
                iid
            ))?;

        let threads = discussions
            .iter()
            .map(|d| d.to_thread())
            .filter(|t| !t.notes.is_empty())
            .collect();

        Ok(threads)
    }

    async fn fetch_user_mrs(&self) -> Result<Vec<UserMrSummary>> {
        let url = format!(
            "{}/merge_requests?scope=created_by_me&state=opened&per_page=100",
            self.base_url
        );
        let mrs: Vec<GitLabUserMr> = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .context(format!("Failed to fetch user MR list from {}", url))?
            .error_for_status()
            .context("GitLab API returned error status for user MR list")?
            .json()
            .await
            .context("Failed to parse user MR list response")?;

        let user_mrs = mrs
            .into_iter()
            .filter_map(|mr| {
                let project_path = extract_gitlab_project_path(&mr.web_url)?;
                Some(UserMrSummary {
                    iid: mr.iid,
                    title: mr.title,
                    web_url: mr.web_url,
                    project_path,
                    author: mr.author,
                    draft: mr.draft,
                    updated_at: mr.updated_at,
                })
            })
            .collect();

        Ok(user_mrs)
    }
}
