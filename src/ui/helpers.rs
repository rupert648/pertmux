use crate::types::{AgentPane, PaneStatus, SessionDetail};
use super::ACCENT;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

// ─── Formatting ──────────────────────────────────────────────────────────────

pub(crate) fn shorten_path(path: &str) -> String {
    if let Some(home) = dirs::home_dir()
        && let Some(rest) = path.strip_prefix(home.to_str().unwrap_or(""))
    {
        return format!("~{}", rest);
    }
    path.to_string()
}

pub(crate) fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

pub(crate) fn format_timestamp(ts_ms: i64) -> String {
    let secs = ts_ms / 1000;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    format!("{:02}:{:02}", hours, mins)
}

pub(crate) fn session_duration(detail: &SessionDetail) -> Option<String> {
    let created = detail.session_created?;
    let updated = detail.session_updated?;
    let elapsed_secs = (updated - created) / 1000;
    if elapsed_secs < 60 {
        Some(format!("{}s", elapsed_secs))
    } else if elapsed_secs < 3600 {
        Some(format!("{}m", elapsed_secs / 60))
    } else if elapsed_secs < 86400 {
        Some(format!(
            "{}h {}m",
            elapsed_secs / 3600,
            (elapsed_secs % 3600) / 60
        ))
    } else {
        Some(format!(
            "{}d {}h",
            elapsed_secs / 86400,
            (elapsed_secs % 86400) / 3600
        ))
    }
}

pub(crate) fn format_date(iso: &str) -> &str {
    if iso.len() >= 10 {
        &iso[..10]
    } else {
        iso
    }
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max > 3 {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{}...", truncated)
    } else {
        s.chars().take(max).collect()
    }
}

pub(crate) fn leak_status(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// Parse ISO 8601 datetime (e.g. "2026-01-02T15:04:05Z") into relative time like "2d ago".
pub(crate) fn format_relative_time(iso: &str) -> String {
    if iso.len() < 19 {
        return iso.to_string();
    }
    let parts: Vec<&str> = iso[..19].split('T').collect();
    if parts.len() != 2 {
        return iso.to_string();
    }
    let date_parts: Vec<u64> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let time_parts: Vec<u64> = parts[1].split(':').filter_map(|s| s.parse().ok()).collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return iso.to_string();
    }

    let (year, month, day) = (date_parts[0], date_parts[1], date_parts[2]);
    let (hour, min, sec) = (time_parts[0], time_parts[1], time_parts[2]);

    let days_from_year = |y: u64| -> u64 {
        365 * y + y / 4 - y / 100 + y / 400
    };
    let days_in_month: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let mut total_days = days_from_year(year) - days_from_year(1970);
    for m in 0..(month.saturating_sub(1) as usize) {
        total_days += if m < 12 { days_in_month[m] } else { 30 };
        if m == 1 && is_leap {
            total_days += 1;
        }
    }
    total_days += day.saturating_sub(1);

    let ts = total_days * 86400 + hour * 3600 + min * 60 + sec;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let delta = (now as i64 - ts as i64).max(0) as u64;

    if delta < 60 {
        "just now".to_string()
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86400 {
        format!("{}h ago", delta / 3600)
    } else if delta < 604800 {
        format!("{}d ago", delta / 86400)
    } else {
        format!("{}w ago", delta / 604800)
    }
}

// ─── Merge status ────────────────────────────────────────────────────────────

pub(crate) fn merge_status_display(
    status: Option<&str>,
    has_conflicts: Option<bool>,
) -> (&'static str, &'static str, Color) {
    if has_conflicts == Some(true) {
        return ("\u{2717}", "conflicts", Color::Red);
    }
    match status {
        Some("mergeable") => ("\u{2713}", "mergeable", Color::Green),
        Some("not_approved") => ("\u{25cb}", "not approved", Color::Yellow),
        Some("checking") => ("\u{29d7}", "checking", ACCENT),
        Some("ci_must_pass") | Some("ci_still_running") => ("\u{29d7}", "CI running", ACCENT),
        Some("broken_status") => ("\u{2717}", "broken", Color::Red),
        Some("need_rebase") => ("\u{21bb}", "needs rebase", Color::Yellow),
        Some("blocked_status") => ("\u{2298}", "blocked", Color::Red),
        Some("discussions_not_resolved") => ("\u{25ce}", "discussions open", Color::Yellow),
        Some("draft_status") => ("\u{25c7}", "draft", Color::DarkGray),
        Some("not_open") => ("\u{2500}", "closed", Color::DarkGray),
        Some(other) => ("?", leak_status(other), Color::DarkGray),
        None => ("\u{2500}", "unknown", Color::DarkGray),
    }
}

// ─── Status badges ───────────────────────────────────────────────────────────

pub(crate) fn status_badge(status: &PaneStatus) -> Span<'static> {
    match status {
        PaneStatus::Busy => Span::styled(
            " \u{25cf} BUSY ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Retry { .. } => Span::styled(
            " \u{26a0} RETRY ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Idle => Span::styled(
            " \u{23f8} IDLE ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Unknown => Span::styled(
            " ? no server ",
            Style::default()
                .fg(Color::DarkGray)
                .bg(Color::Indexed(236)),
        ),
    }
}

pub(crate) fn compact_status_badge(status: &PaneStatus) -> Span<'static> {
    match status {
        PaneStatus::Busy => Span::styled(
            " B ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Retry { .. } => Span::styled(
            " R ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Idle => Span::styled(
            " I ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Unknown => Span::styled("   ", Style::default()),
    }
}

// ─── Scroll ──────────────────────────────────────────────────────────────────

pub(crate) fn compute_scroll(
    lines: &[Line],
    selected: usize,
    groups: &[(String, Vec<usize>)],
    panes: &[AgentPane],
    visible_height: usize,
) -> usize {
    let mut line_idx = 0;
    let mut flat = 0;
    for (_, pane_indices) in groups {
        line_idx += 1;
        for &idx in pane_indices {
            if flat == selected {
                if line_idx + 3 > visible_height {
                    return line_idx.saturating_sub(visible_height / 2);
                }
                return 0;
            }
            let pane_lines = if panes[idx].last_response.is_some() {
                3
            } else {
                2
            };
            line_idx += pane_lines;
            flat += 1;
        }
        line_idx += 1;
    }
    if lines.len() > visible_height {
        lines.len() - visible_height
    } else {
        0
    }
}
