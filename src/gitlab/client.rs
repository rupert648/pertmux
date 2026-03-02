use crate::gitlab::types::*;
use anyhow::{Context, Result};
use reqwest::Client;

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

    pub async fn fetch_mr_list(&self) -> Result<Vec<MergeRequestSummary>> {
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

    pub async fn fetch_mr_detail(&self, mr_iid: u64) -> Result<MergeRequestDetail> {
        let url = format!(
            "{}/projects/{}/merge_requests/{}",
            self.base_url, self.project_id, mr_iid
        );
        self.client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .context(format!("Failed to fetch MR detail from {}", url))?
            .error_for_status()
            .context(format!(
                "GitLab API returned error status for MR {}",
                mr_iid
            ))?
            .json::<MergeRequestDetail>()
            .await
            .context(format!(
                "Failed to parse MR detail response for {}",
                mr_iid
            ))
    }

    pub async fn fetch_mr_notes(&self, mr_iid: u64) -> Result<Vec<MergeRequestNote>> {
        let url = format!(
            "{}/projects/{}/merge_requests/{}/notes?per_page=100",
            self.base_url, self.project_id, mr_iid
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
                mr_iid
            ))?
            .json::<Vec<MergeRequestNote>>()
            .await
            .context(format!(
                "Failed to parse MR notes response for {}",
                mr_iid
            ))
    }
}
