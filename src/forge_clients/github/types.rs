use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GhUser {
    pub id: u64,
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GhPrRef {
    #[serde(rename = "ref")]
    pub branch: String,
    pub sha: String,
}

#[derive(Debug, Deserialize)]
pub struct GhPullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub head: GhPrRef,
    pub base: GhPrRef,
    pub user: GhUser,
    pub draft: bool,
    #[serde(default)]
    pub comments: u32,
    #[serde(default)]
    pub review_comments: u32,
    pub html_url: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub mergeable: Option<bool>,
    #[serde(default)]
    pub mergeable_state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GhCheckRunApp {
    #[serde(default)]
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GhCheckRun {
    pub id: u64,
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub app: Option<GhCheckRunApp>,
}

#[derive(Debug, Deserialize)]
pub struct GhCheckRunsResponse {
    #[allow(dead_code)]
    pub total_count: u64,
    pub check_runs: Vec<GhCheckRun>,
}

#[derive(Debug, Deserialize)]
pub struct GhIssueComment {
    pub id: u64,
    pub body: Option<String>,
    pub user: GhUser,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gh_pull_request_deserializes() {
        let json = r#"{
            "number": 1,
            "title": "feat: add thing",
            "state": "open",
            "head": {"ref": "feat/thing", "sha": "abc123"},
            "base": {"ref": "main", "sha": "def456"},
            "user": {"id": 1, "login": "rupert"},
            "draft": false,
            "comments": 3,
            "review_comments": 2,
            "html_url": "https://github.com/org/repo/pull/1",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z",
            "mergeable": true,
            "mergeable_state": "clean"
        }"#;
        let pr: GhPullRequest = serde_json::from_str(json).unwrap();
        assert_eq!(pr.number, 1);
        assert_eq!(pr.head.branch, "feat/thing");
        assert_eq!(pr.head.sha, "abc123");
        assert_eq!(pr.state, "open");
        assert_eq!(pr.user.login, "rupert");
        assert!(!pr.draft);
        assert_eq!(pr.comments + pr.review_comments, 5);
    }

    #[test]
    fn test_gh_check_runs_response_deserializes() {
        let json = r#"{
            "total_count": 2,
            "check_runs": [
                {
                    "id": 100,
                    "name": "build",
                    "status": "completed",
                    "conclusion": "success",
                    "app": {"slug": "github-actions"}
                },
                {
                    "id": 101,
                    "name": "test",
                    "status": "in_progress",
                    "conclusion": null,
                    "app": {"slug": "github-actions"}
                }
            ]
        }"#;
        let resp: GhCheckRunsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.check_runs.len(), 2);
        assert_eq!(resp.check_runs[0].name, "build");
        assert_eq!(resp.check_runs[0].conclusion, Some("success".to_string()));
        assert_eq!(resp.check_runs[1].status, "in_progress");
        assert!(resp.check_runs[1].conclusion.is_none());
    }

    #[test]
    fn test_gh_pr_ref_rename() {
        let json = r#"{"ref": "my-branch", "sha": "deadbeef"}"#;
        let pr_ref: GhPrRef = serde_json::from_str(json).unwrap();
        assert_eq!(pr_ref.branch, "my-branch");
        assert_eq!(pr_ref.sha, "deadbeef");
    }

    #[test]
    fn test_gh_pr_minimal_fields() {
        let json = r#"{
            "number": 42,
            "title": "test",
            "state": "open",
            "head": {"ref": "feat", "sha": "aaa"},
            "base": {"ref": "main", "sha": "bbb"},
            "user": {"id": 1, "login": "user"},
            "draft": false,
            "comments": 0,
            "html_url": "https://github.com/o/r/pull/42",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }"#;
        let pr: GhPullRequest = serde_json::from_str(json).unwrap();
        assert_eq!(pr.number, 42);
        assert!(pr.mergeable.is_none());
        assert!(pr.mergeable_state.is_none());
        assert_eq!(pr.review_comments, 0);
    }

    #[test]
    fn test_gh_issue_comment_deserializes() {
        let json = r#"{
            "id": 200,
            "body": "LGTM!",
            "user": {"id": 2, "login": "reviewer"},
            "created_at": "2026-01-02T00:00:00Z"
        }"#;
        let comment: GhIssueComment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.id, 200);
        assert_eq!(comment.body, Some("LGTM!".to_string()));
        assert_eq!(comment.user.login, "reviewer");
    }
}
