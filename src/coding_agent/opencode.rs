use super::CodingAgent;
use crate::discovery;
use crate::types::PaneStatus;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

pub struct OpenCode;

// ─── Opencode-specific API types ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SessionStatus {
    #[serde(rename = "type")]
    status_type: String,
    attempt: Option<u32>,
    message: Option<String>,
}

type SessionStatusMap = HashMap<String, SessionStatus>;

// ─── Trait implementation ────────────────────────────────────────────────────

const TIMEOUT: Duration = Duration::from_secs(1);

impl CodingAgent for OpenCode {
    fn name(&self) -> &str {
        "opencode"
    }

    fn process_name(&self) -> &str {
        "opencode"
    }

    fn query_status(&self, pane_pid: u32) -> PaneStatus {
        let Some(port) = discovery::discover_port(pane_pid) else {
            return PaneStatus::Unknown;
        };

        let Some(map) = get_session_status(port) else {
            return PaneStatus::Unknown;
        };

        status_from_map(&map)
    }
}

// ─── Internal helpers ────────────────────────────────────────────────────────

fn get_session_status(port: u16) -> Option<SessionStatusMap> {
    let url = format!("http://127.0.0.1:{}/session/status", port);
    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_connect(Some(TIMEOUT))
            .timeout_recv_body(Some(TIMEOUT))
            .build(),
    );
    let mut response = agent.get(&url).call().ok()?;
    response.body_mut().read_json::<SessionStatusMap>().ok()
}

/// Determine overall status from the opencode API response.
/// Priority: Busy > Retry > Idle.
fn status_from_map(map: &SessionStatusMap) -> PaneStatus {
    if map.is_empty() {
        return PaneStatus::Idle;
    }
    for status in map.values() {
        if status.status_type == "busy" {
            return PaneStatus::Busy;
        }
    }
    for status in map.values() {
        if status.status_type == "retry" {
            return PaneStatus::Retry {
                attempt: status.attempt.unwrap_or(0),
                message: status.message.clone().unwrap_or_default(),
            };
        }
    }
    PaneStatus::Idle
}
