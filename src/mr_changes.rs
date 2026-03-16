use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MrChange {
    pub project_name: String,
    pub mr_iid: u64,
    pub mr_title: String,
    pub change_type: MrChangeType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MrChangeType {
    PipelineFailed,
    PipelineSucceeded,
    NewDiscussions(u32),
    Approved,
}

impl fmt::Display for MrChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let action = match &self.change_type {
            MrChangeType::PipelineFailed => "Pipeline failed".to_string(),
            MrChangeType::PipelineSucceeded => "Pipeline succeeded".to_string(),
            MrChangeType::NewDiscussions(n) => {
                if *n == 1 {
                    "1 new discussion".to_string()
                } else {
                    format!("{} new discussions", n)
                }
            }
            MrChangeType::Approved => "Approved".to_string(),
        };
        write!(f, "{} !{}: {}", self.project_name, self.mr_iid, action)
    }
}
