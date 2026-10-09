use crate::client::ClientState;
use crate::protocol::{ActivityContext, ActivityEntry, ActivityKind, ActivityTarget};
use crate::types::AgentPane;
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

    let visible = (inner.height as usize / ACTIVITY_CARD_HEIGHT).max(1);

    let items: Vec<ListItem> = state
        .snapshot
        .activity_feed
        .iter()
        .take(visible)
        .map(|entry| {
            ListItem::new(activity_card_lines(
                entry,
                activity_pane(state, entry),
                inner.width,
                false,
            ))
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}

pub(crate) const ACTIVITY_STATUS_WIDTH: usize = 12;
pub(crate) const ACTIVITY_CARD_HEIGHT: usize = 4;

pub(crate) fn activity_status_label(kind: &ActivityKind) -> &'static str {
    match kind {
        ActivityKind::AgentBusy => "WORKING",
        ActivityKind::AgentIdle => "NEEDS ACTION",
        ActivityKind::AgentRetry => "RETRYING",
        ActivityKind::AgentHandled => "HANDLED",
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
        ActivityKind::AgentHandled => Style::default()
            .fg(Color::Rgb(100, 200, 140))
            .add_modifier(Modifier::BOLD),
        ActivityKind::MrPipelineFailed
        | ActivityKind::MrPipelineSucceeded
        | ActivityKind::MrNewDiscussions
        | ActivityKind::MrApproved => Style::default().fg(Color::DarkGray),
    }
}

pub(crate) fn activity_pane<'a>(
    state: &'a ClientState,
    entry: &ActivityEntry,
) -> Option<&'a AgentPane> {
    let Some(ActivityTarget::Pane { pane_id, pane_path }) = &entry.target else {
        return None;
    };

    state
        .snapshot
        .panes
        .iter()
        .find(|pane| pane.pane_id == *pane_id)
        .or_else(|| {
            state.snapshot.panes.iter().find(|pane| {
                pane.pane_path.trim_end_matches('/') == pane_path.trim_end_matches('/')
            })
        })
}

pub(crate) fn activity_card_lines(
    entry: &ActivityEntry,
    pane: Option<&AgentPane>,
    width: u16,
    selected: bool,
) -> Vec<Line<'static>> {
    let width = width as usize;
    let border_style = if selected {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Indexed(239))
    };
    let name_style = if selected {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let status = activity_status_label(&entry.kind);
    let time = activity_time_ago(entry);
    let name_width = width.saturating_sub(18 + time.chars().count()).max(1);
    let title_width = width.saturating_sub(4).max(1);

    let title = pane
        .map(AgentPane::display_title)
        .filter(|title| !title.trim().is_empty())
        .map(single_line)
        .or_else(|| {
            entry
                .context
                .as_ref()
                .and_then(|context| context.session_title.as_deref())
                .filter(|title| !title.trim().is_empty())
                .map(single_line)
        })
        .unwrap_or_else(|| "No Codex session title yet".to_string());
    let response = pane
        .and_then(|pane| pane.last_response.as_deref())
        .or_else(|| {
            entry
                .context
                .as_ref()
                .and_then(|context| context.last_response.as_deref())
        })
        .filter(|response| !response.trim().is_empty())
        .map(|response| format!("↳ {}", single_line(response)))
        .unwrap_or_else(|| "↳ No response preview".to_string());
    let metadata = pane
        .map(activity_metadata)
        .or_else(|| entry.context.as_ref().map(activity_context_metadata))
        .unwrap_or_else(|| "Agent pane is no longer active".to_string());

    vec![
        Line::from(vec![
            Span::styled(if selected { "╭▸" } else { "╭ " }, border_style),
            Span::styled(
                fit_text(status, ACTIVITY_STATUS_WIDTH),
                activity_status_style(&entry.kind),
            ),
            Span::raw(" "),
            Span::styled(fit_text(&entry.label, name_width), name_style),
            Span::raw(" "),
            Span::styled(time, Style::default().fg(Color::Indexed(245))),
            Span::styled(" ╮", border_style),
        ]),
        framed_card_line(
            &title,
            title_width,
            if selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            },
            border_style,
            "│ ",
            " │",
        ),
        framed_card_line(
            &response,
            title_width,
            Style::default().fg(Color::Indexed(245)),
            border_style,
            "│ ",
            " │",
        ),
        framed_card_line(
            &metadata,
            title_width,
            Style::default().fg(Color::DarkGray),
            border_style,
            "╰ ",
            " ╯",
        ),
    ]
}

fn framed_card_line(
    content: &str,
    width: usize,
    content_style: Style,
    border_style: Style,
    left: &'static str,
    right: &'static str,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(left, border_style),
        Span::styled(fit_text(content, width), content_style),
        Span::styled(right, border_style),
    ])
}

fn activity_metadata(pane: &AgentPane) -> String {
    let mut parts = Vec::new();
    if let Some(agent) = pane.agent.as_deref() {
        parts.push(agent.to_string());
    }
    if let Some(model) = pane.model.as_deref() {
        parts.push(model.to_string());
    }
    parts.push(format!("tmux:{}", pane.session_name));
    parts.join(" · ")
}

fn activity_context_metadata(context: &ActivityContext) -> String {
    let mut parts = Vec::new();
    if let Some(agent) = context.agent.as_deref() {
        parts.push(agent.to_string());
    }
    if let Some(model) = context.model.as_deref() {
        parts.push(model.to_string());
    }
    if let Some(session) = context.tmux_session.as_deref() {
        parts.push(format!("tmux:{session}"));
    }
    if parts.is_empty() {
        "Saved activity".to_string()
    } else {
        parts.join(" · ")
    }
}

fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fit_text(value: &str, width: usize) -> String {
    let value = truncate_to(value, width);
    let padding = width.saturating_sub(value.chars().count());
    format!("{value}{}", " ".repeat(padding))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ActivityTarget;
    use crate::types::PaneStatus;

    #[test]
    fn card_includes_codex_context_and_fits_requested_width() {
        let pane = AgentPane {
            pane_id: "%1".to_string(),
            session_name: "pertmux".to_string(),
            window_index: 1,
            pane_index: 0,
            pane_title: "codex".to_string(),
            pane_path: "/tmp/feature".to_string(),
            pane_pid: 42,
            pane_command: "codex".to_string(),
            status: PaneStatus::Idle,
            db_session_title: Some("Improve the agent activity cards".to_string()),
            agent: Some("codex".to_string()),
            model: Some("gpt-6".to_string()),
            last_activity: None,
            status_changed_at: None,
            db_session_id: Some("session-1".to_string()),
            last_response: Some("Implemented the requested interaction".to_string()),
        };
        let entry = ActivityEntry {
            label: "feature".to_string(),
            message: "handled".to_string(),
            kind: ActivityKind::AgentHandled,
            received_at_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            target: Some(ActivityTarget::Pane {
                pane_id: pane.pane_id.clone(),
                pane_path: pane.pane_path.clone(),
            }),
            context: Some(ActivityContext::from(&pane)),
        };

        let lines = activity_card_lines(&entry, None, 72, true);
        let rendered = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert_eq!(lines.len(), ACTIVITY_CARD_HEIGHT);
        assert!(rendered.contains("HANDLED"));
        assert!(rendered.contains("Improve the agent activity cards"));
        assert!(rendered.contains("Implemented the requested interaction"));
        assert!(rendered.contains("codex · gpt-6 · tmux:pertmux"));
        assert!(lines.iter().all(|line| {
            line.spans
                .iter()
                .map(|span| span.content.chars().count())
                .sum::<usize>()
                == 72
        }));
    }
}
