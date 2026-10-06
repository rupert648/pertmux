use crate::client::ClientState;
use crate::protocol::{ActivityEntry, ActivityKind};
use crate::ui::ACCENT;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Padding},
};

pub(crate) fn draw_activity_feed(frame: &mut Frame, state: &ClientState, area: Rect) {
    let kb_hint = state.snapshot.keybindings.activity_feed;
    let title = format!(" Agent Activity [{}] ", kb_hint);
    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(Color::DarkGray)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Indexed(235)))
        .padding(Padding::new(1, 1, 0, 0));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.snapshot.activity_feed.is_empty() {
        let placeholder = ListItem::new(Line::from(Span::styled(
            "no agent activity yet",
            Style::default().fg(Color::Indexed(237)),
        )));
        frame.render_widget(List::new(vec![placeholder]), inner);
        return;
    }

    let visible = inner.height as usize;
    let available_width = inner.width as usize;

    let items: Vec<ListItem> = state
        .snapshot
        .activity_feed
        .iter()
        .take(visible)
        .map(|entry| {
            let status = activity_status_label(&entry.kind);
            let time = activity_time_ago(entry);
            let max_label = available_width
                .saturating_sub(ACTIVITY_STATUS_WIDTH + time.len() + 2)
                .max(1);
            let label = truncate_to(&entry.label, max_label);

            Line::from(vec![
                Span::styled(
                    format!("{:<width$} ", status, width = ACTIVITY_STATUS_WIDTH),
                    activity_status_style(&entry.kind),
                ),
                Span::styled(
                    format!("{:<width$} ", label, width = max_label),
                    Style::default().fg(Color::White),
                ),
                Span::styled(time, Style::default().fg(Color::Indexed(245))),
            ])
            .into()
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}

pub(crate) const ACTIVITY_STATUS_WIDTH: usize = 12;

pub(crate) fn activity_status_label(kind: &ActivityKind) -> &'static str {
    match kind {
        ActivityKind::AgentBusy => "WORKING",
        ActivityKind::AgentIdle => "NEEDS ACTION",
        ActivityKind::AgentRetry => "RETRYING",
        ActivityKind::MrPipelineFailed
        | ActivityKind::MrPipelineSucceeded
        | ActivityKind::MrNewDiscussions
        | ActivityKind::MrApproved => "UPDATE",
    }
}

pub(crate) fn activity_status_style(kind: &ActivityKind) -> Style {
    match kind {
        ActivityKind::AgentBusy => Style::default().fg(ACCENT),
        ActivityKind::AgentIdle => Style::default()
            .fg(Color::Rgb(190, 255, 100))
            .add_modifier(Modifier::BOLD),
        ActivityKind::AgentRetry => Style::default()
            .fg(Color::Rgb(255, 90, 90))
            .add_modifier(Modifier::BOLD),
        ActivityKind::MrPipelineFailed
        | ActivityKind::MrPipelineSucceeded
        | ActivityKind::MrNewDiscussions
        | ActivityKind::MrApproved => Style::default().fg(Color::DarkGray),
    }
}

/// Format elapsed time since an activity entry was recorded by the daemon.
pub(crate) fn activity_time_ago(entry: &ActivityEntry) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now.saturating_sub(entry.received_at_secs);
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

/// Truncate a string to at most `max_chars` characters.
fn truncate_to(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else if max_chars > 1 {
        let truncated: String = chars[..max_chars - 1].iter().collect();
        format!("{}…", truncated)
    } else {
        chars[..max_chars].iter().collect()
    }
}
