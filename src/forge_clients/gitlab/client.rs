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
    // `draft` was added in GitLab 13.2; older instances return `work_in_progress` instead.
    // Accept either name and default to false if neither is present.
    #[serde(default, alias = "work_in_progress")]
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
        let mut all_mrs: Vec<GitLabUserMr> = Vec::new();
        let mut page = 1u32;

        loop {
            let url = format!(
                "{}/merge_requests?scope=created_by_me&state=opened&per_page=100&page={}",
                self.base_url, page
            );
            let response = self
                .client
                .get(&url)
                .header("PRIVATE-TOKEN", &self.token)
                .send()
                .await
                .context(format!("Failed to fetch user MR list from {}", url))?
                .error_for_status()
                .context("GitLab API returned error status for user MR list")?;

            // GitLab sets x-next-page to the next page number, or empty string on the last page.
            let has_next_page = response
                .headers()
                .get("x-next-page")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|s| !s.is_empty());

            let mrs: Vec<GitLabUserMr> = response
                .json()
                .await
                .context("Failed to parse user MR list response")?;

            let fetched = mrs.len();
            all_mrs.extend(mrs);

            if !has_next_page || fetched == 0 {
                break;
            }
            page += 1;
        }

        let user_mrs = all_mrs
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- extract_gitlab_project_path ---

    #[test]
    fn test_extract_path_simple_group() {
        let url = "https://gitlab.example.com/group/project/-/merge_requests/42";
        assert_eq!(
            extract_gitlab_project_path(url),
            Some("group/project".to_string())
        );
    }

    #[test]
    fn test_extract_path_nested_groups() {
        let url = "https://gitlab.example.com/org/team/sub/project/-/merge_requests/1";
        assert_eq!(
            extract_gitlab_project_path(url),
            Some("org/team/sub/project".to_string())
        );
    }

    #[test]
    fn test_extract_path_no_group() {
        let url = "https://gitlab.example.com/project/-/merge_requests/7";
        assert_eq!(
            extract_gitlab_project_path(url),
            Some("project".to_string())
        );
    }

    #[test]
    fn test_extract_path_no_separator_returns_none() {
        // Old-style GitLab URLs without /-/ are not matched
        let url = "https://gitlab.example.com/group/project/merge_requests/42";
        assert_eq!(extract_gitlab_project_path(url), None);
    }

    // --- GitLabUserMr draft field deserialization ---

    fn make_mr_json(draft_field: &str) -> String {
        format!(
            r#"{{
                "iid": 10,
                "title": "My MR",
                "web_url": "https://gitlab.example.com/org/repo/-/merge_requests/10",
                "author": {{"id": 1, "username": "dev", "name": "Developer"}},
                "updated_at": "2026-01-01T00:00:00.000Z"
                {draft_field}
            }}"#
        )
    }

    #[test]
    fn test_deserialize_with_draft_true() {
        // Modern GitLab (13.2+): returns "draft" field
        let json = make_mr_json(r#", "draft": true"#);
        let mr: GitLabUserMr = serde_json::from_str(&json).unwrap();
        assert!(mr.draft);
    }

    #[test]
    fn test_deserialize_with_draft_false() {
        let json = make_mr_json(r#", "draft": false"#);
        let mr: GitLabUserMr = serde_json::from_str(&json).unwrap();
        assert!(!mr.draft);
    }

    #[test]
    fn test_deserialize_with_work_in_progress_true() {
        // Old GitLab (< 13.2): only "work_in_progress" field
        let json = make_mr_json(r#", "work_in_progress": true"#);
        let mr: GitLabUserMr = serde_json::from_str(&json).unwrap();
        assert!(mr.draft);
    }

    #[test]
    fn test_deserialize_with_work_in_progress_false() {
        let json = make_mr_json(r#", "work_in_progress": false"#);
        let mr: GitLabUserMr = serde_json::from_str(&json).unwrap();
        assert!(!mr.draft);
    }

    #[test]
    fn test_deserialize_without_draft_field_defaults_to_false() {
        // Neither field present: should default to false, not fail
        let json = make_mr_json("");
        let mr: GitLabUserMr = serde_json::from_str(&json).unwrap();
        assert!(!mr.draft);
    }

    #[test]
    fn test_deserialize_array_with_mixed_formats() {
        // The critical regression test: a Vec containing MRs with and without
        // the draft field must not fail even if some elements omit it.
        let json = r#"[
            {
                "iid": 1, "title": "Old MR",
                "web_url": "https://gitlab.example.com/a/b/-/merge_requests/1",
                "author": {"id": 1, "username": "dev", "name": "Dev"},
                "updated_at": "2026-01-01T00:00:00Z",
                "work_in_progress": false
            },
            {
                "iid": 2, "title": "New MR",
                "web_url": "https://gitlab.example.com/a/b/-/merge_requests/2",
                "author": {"id": 1, "username": "dev", "name": "Dev"},
                "updated_at": "2026-01-02T00:00:00Z",
                "draft": true
            },
            {
                "iid": 3, "title": "Bare MR",
                "web_url": "https://gitlab.example.com/a/b/-/merge_requests/3",
                "author": {"id": 1, "username": "dev", "name": "Dev"},
                "updated_at": "2026-01-03T00:00:00Z"
            }
        ]"#;
        let mrs: Vec<GitLabUserMr> = serde_json::from_str(json).unwrap();
        assert_eq!(mrs.len(), 3);
        assert!(!mrs[0].draft);
        assert!(mrs[1].draft);
        assert!(!mrs[2].draft);
    }
}
