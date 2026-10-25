use async_trait::async_trait;

use crate::forge_clients::types::{
    MergeRequestDetail, MergeRequestNote, MergeRequestSummary, MergeRequestThread, PipelineJob,
    UserMrSummary,
};

// `async_trait` marks its generated futures as must-use; current Clippy also
// infers must-use from `Future`, producing `double_must_use` on each method.
#[allow(clippy::double_must_use)]
#[async_trait(?Send)]
pub trait ForgeClient {
    async fn fetch_mrs(&self) -> anyhow::Result<Vec<MergeRequestSummary>>;
    async fn fetch_mr_detail(&self, iid: u64) -> anyhow::Result<MergeRequestDetail>;
    async fn fetch_ci_jobs(
        &self,
        mr_detail: &MergeRequestDetail,
    ) -> anyhow::Result<Vec<PipelineJob>>;
    async fn fetch_notes(&self, iid: u64) -> anyhow::Result<Vec<MergeRequestNote>>;
    async fn fetch_discussions(&self, iid: u64) -> anyhow::Result<Vec<MergeRequestThread>>;
    async fn fetch_user_mrs(&self) -> anyhow::Result<Vec<UserMrSummary>>;
}
