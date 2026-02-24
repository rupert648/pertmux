use crate::types::{OpenCodeStatus, SessionStatusMap};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(1);

pub fn get_session_status(port: u16) -> Option<SessionStatusMap> {
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

/// Determine the overall status for a pane from the API response.
/// If ANY session is busy, the pane is busy.
/// If any is retrying, it's retrying.
/// Otherwise idle.
pub fn status_from_map(map: &Option<SessionStatusMap>) -> OpenCodeStatus {
    let Some(map) = map else {
        return OpenCodeStatus::Unknown;
    };
    // Empty map = all sessions idle
    if map.is_empty() {
        return OpenCodeStatus::Idle;
    }
    // Check for busy first (highest priority)
    for status in map.values() {
        if status.status_type == "busy" {
            return OpenCodeStatus::Busy;
        }
    }
    // Check for retry
    for status in map.values() {
        if status.status_type == "retry" {
            return OpenCodeStatus::Retry {
                attempt: status.attempt.unwrap_or(0),
                message: status.message.clone().unwrap_or_default(),
            };
        }
    }
    OpenCodeStatus::Idle
}
