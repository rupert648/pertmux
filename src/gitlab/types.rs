use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct GitLabUser {
    pub id: u64,
    pub username: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PipelineInfo {
    pub id: u64,
    pub status: String,
    pub web_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeRequestSummary {
    pub iid: u64,
    pub title: String,
    pub state: String,
    pub source_branch: String,
    pub target_branch: String,
    pub author: GitLabUser,
    pub draft: bool,
    pub user_notes_count: u32,
    pub web_url: String,
    pub created_at: String,
    pub updated_at: String,
    pub detailed_merge_status: Option<String>,
    pub has_conflicts: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeRequestDetail {
    pub iid: u64,
    pub title: String,
    pub state: String,
    pub source_branch: String,
    pub target_branch: String,
    pub author: GitLabUser,
    pub draft: bool,
    pub user_notes_count: u32,
    pub web_url: String,
    pub created_at: String,
    pub updated_at: String,
    pub detailed_merge_status: Option<String>,
    pub has_conflicts: Option<bool>,
    pub assignees: Vec<GitLabUser>,
    pub reviewers: Vec<GitLabUser>,
    pub head_pipeline: Option<PipelineInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MergeRequestNote {
    pub id: u64,
    pub body: String,
    pub author: GitLabUser,
    pub created_at: String,
    pub system: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PipelineJob {
    pub id: u64,
    pub name: String,
    pub stage: String,
    pub status: String,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub allow_failure: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const MR_LIST_JSON: &str = r#"[
        {
            "iid": 42,
            "title": "feat: add authentication",
            "state": "opened",
            "source_branch": "feat/auth",
            "target_branch": "main",
            "author": {"id": 1, "username": "rupert", "name": "Rupert Carr"},
            "draft": false,
            "user_notes_count": 5,
            "web_url": "https://gitlab.example.com/team/project/-/merge_requests/42",
            "created_at": "2026-01-01T00:00:00.000Z",
            "updated_at": "2026-01-02T00:00:00.000Z",
            "detailed_merge_status": "mergeable",
            "has_conflicts": false
        }
    ]"#;

    const MR_DETAIL_JSON: &str = r#"{
        "iid": 42,
        "title": "feat: add authentication",
        "state": "opened",
        "source_branch": "feat/auth",
        "target_branch": "main",
        "author": {"id": 1, "username": "rupert", "name": "Rupert Carr"},
        "draft": false,
        "user_notes_count": 5,
        "web_url": "https://gitlab.example.com/team/project/-/merge_requests/42",
        "created_at": "2026-01-01T00:00:00.000Z",
        "updated_at": "2026-01-02T00:00:00.000Z",
        "detailed_merge_status": "mergeable",
        "has_conflicts": false,
        "assignees": [],
        "reviewers": [{"id": 2, "username": "alice", "name": "Alice"}],
        "head_pipeline": {"id": 999, "status": "success", "web_url": "https://gitlab.example.com/..."}
    }"#;

    const MR_NOTES_JSON: &str = r#"[
        {
            "id": 100,
            "body": "Looks good to me!",
            "author": {"id": 2, "username": "alice", "name": "Alice"},
            "created_at": "2026-01-02T00:00:00.000Z",
            "system": false
        },
        {
            "id": 101,
            "body": "added label ~review",
            "author": {"id": 0, "username": "system", "name": "System"},
            "created_at": "2026-01-02T01:00:00.000Z",
            "system": true
        }
    ]"#;

    #[test]
    fn test_mr_list_deserializes() {
        let mrs: Vec<MergeRequestSummary> = serde_json::from_str(MR_LIST_JSON).unwrap();
        assert_eq!(mrs.len(), 1);
        assert_eq!(mrs[0].iid, 42);
        assert_eq!(mrs[0].source_branch, "feat/auth");
        assert_eq!(mrs[0].state, "opened");
        assert!(!mrs[0].draft);
        assert_eq!(mrs[0].author.username, "rupert");
    }

    #[test]
    fn test_mr_detail_deserializes() {
        let mr: MergeRequestDetail = serde_json::from_str(MR_DETAIL_JSON).unwrap();
        assert_eq!(mr.iid, 42);
        assert_eq!(mr.detailed_merge_status, Some("mergeable".to_string()));
        assert_eq!(mr.has_conflicts, Some(false));
        assert_eq!(mr.reviewers.len(), 1);
        assert_eq!(mr.reviewers[0].username, "alice");
        let pipeline = mr.head_pipeline.unwrap();
        assert_eq!(pipeline.status, "success");
    }

    #[test]
    fn test_mr_detail_null_pipeline() {
        let json = r#"{
            "iid": 1, "title": "test", "state": "opened",
            "source_branch": "feat", "target_branch": "main",
            "author": {"id": 1, "username": "u", "name": "User"},
            "draft": false, "user_notes_count": 0,
            "web_url": "https://x", "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "detailed_merge_status": null, "has_conflicts": null,
            "assignees": [], "reviewers": [], "head_pipeline": null
        }"#;
        let mr: MergeRequestDetail = serde_json::from_str(json).unwrap();
        assert!(mr.head_pipeline.is_none());
        assert!(mr.detailed_merge_status.is_none());
    }

    #[test]
    fn test_mr_notes_system_flag() {
        let notes: Vec<MergeRequestNote> = serde_json::from_str(MR_NOTES_JSON).unwrap();
        assert_eq!(notes.len(), 2);
        assert!(!notes[0].system);
        assert!(notes[1].system);
    }

    #[test]
    fn test_mr_list_with_draft_mr() {
        let json = r#"[
            {
                "iid": 55,
                "title": "Draft: my feature",
                "state": "opened",
                "source_branch": "feat/draft-feature",
                "target_branch": "main",
                "author": {"id": 1, "username": "dev", "name": "Developer"},
                "draft": true,
                "user_notes_count": 0,
                "web_url": "https://gitlab.example.com/team/project/-/merge_requests/55",
                "created_at": "2026-02-01T00:00:00.000Z",
                "updated_at": "2026-02-01T00:00:00.000Z",
                "detailed_merge_status": null,
                "has_conflicts": null
            }
        ]"#;
        let mrs: Vec<MergeRequestSummary> = serde_json::from_str(json).unwrap();
        assert_eq!(mrs.len(), 1);
        assert!(mrs[0].draft);
        assert_eq!(mrs[0].title, "Draft: my feature");
        assert_eq!(mrs[0].iid, 55);
    }

    #[test]
    fn test_mr_notes_empty_list() {
        let notes: Vec<MergeRequestNote> = serde_json::from_str("[]").unwrap();
        assert!(notes.is_empty());
    }

    const PIPELINE_JOBS_JSON: &str = r#"[
        {
            "id": 1001,
            "name": "lint",
            "stage": "build",
            "status": "success",
            "duration": 45.2,
            "allow_failure": false
        },
        {
            "id": 1002,
            "name": "compile",
            "stage": "build",
            "status": "success",
            "duration": 120.5,
            "allow_failure": false
        },
        {
            "id": 1003,
            "name": "unit-tests",
            "stage": "test",
            "status": "failed",
            "duration": 89.1,
            "allow_failure": false
        },
        {
            "id": 1004,
            "name": "integration-tests",
            "stage": "test",
            "status": "running",
            "duration": null,
            "allow_failure": true
        }
    ]"#;

    #[test]
    fn test_pipeline_jobs_deserializes() {
        let jobs: Vec<PipelineJob> = serde_json::from_str(PIPELINE_JOBS_JSON).unwrap();
        assert_eq!(jobs.len(), 4);
        assert_eq!(jobs[0].name, "lint");
        assert_eq!(jobs[0].stage, "build");
        assert_eq!(jobs[0].status, "success");
        assert!(jobs[0].duration.is_some());
        assert!(!jobs[0].allow_failure);
        assert_eq!(jobs[2].status, "failed");
        assert!(!jobs[2].allow_failure);
        assert!(jobs[3].allow_failure);
        assert!(jobs[3].duration.is_none());
    }

    #[test]
    fn test_pipeline_jobs_missing_optional_fields() {
        let json = r#"[{
            "id": 1,
            "name": "deploy",
            "stage": "deploy",
            "status": "created"
        }]"#;
        let jobs: Vec<PipelineJob> = serde_json::from_str(json).unwrap();
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].duration.is_none());
        assert!(!jobs[0].allow_failure);
    }

    #[test]
    fn test_mr_detail_with_conflicts_and_assignees() {
        let json = r#"{
            "iid": 88,
            "title": "fix: resolve merge conflict",
            "state": "opened",
            "source_branch": "fix/conflicts",
            "target_branch": "main",
            "author": {"id": 1, "username": "dev", "name": "Developer"},
            "draft": false,
            "user_notes_count": 3,
            "web_url": "https://gitlab.example.com/team/project/-/merge_requests/88",
            "created_at": "2026-03-01T00:00:00.000Z",
            "updated_at": "2026-03-01T12:00:00.000Z",
            "detailed_merge_status": "broken_status",
            "has_conflicts": true,
            "assignees": [
                {"id": 2, "username": "alice", "name": "Alice"},
                {"id": 3, "username": "bob", "name": "Bob"}
            ],
            "reviewers": [],
            "head_pipeline": null
        }"#;
        let mr: MergeRequestDetail = serde_json::from_str(json).unwrap();
        assert_eq!(mr.has_conflicts, Some(true));
        assert_eq!(mr.assignees.len(), 2);
        assert_eq!(mr.assignees[0].username, "alice");
        assert_eq!(mr.assignees[1].username, "bob");
        assert!(mr.head_pipeline.is_none());
    }
}
