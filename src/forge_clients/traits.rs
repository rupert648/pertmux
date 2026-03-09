use async_trait::async_trait;

use crate::forge_clients::types::{
    MergeRequestDetail, MergeRequestNote, MergeRequestSummary, MergeRequestThread, PipelineJob,
};

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
}
