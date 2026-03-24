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
    let title = format!(" Activity [{}] ", kb_hint);
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
            "no activity yet",
            Style::default().fg(Color::Indexed(237)),
        )));
        frame.render_widget(List::new(vec![placeholder]), inner);
        return;
    }

    // How many rows fit in the inner area
    let visible = inner.height as usize;

    // Reserve: 2 (node+space) + 21 (message 20+space) + 5 (time up to "120m") = 28 chars.
    // Give the remainder to the label so long worktree names are not cut off.
    let max_label = (inner.width as usize).saturating_sub(28).max(8);

    let items: Vec<ListItem> = state
        .snapshot
        .activity_feed
        .iter()
        .take(visible)
        .map(|entry| {
            let r = recency(entry);
            let base = kind_base_color(&entry.kind);
            let (node, node_style) = node_for_recency(r, base);
            let (label_style, msg_style, time_style) = text_styles_for_recency(r, base);

            let label = truncate_to(&entry.label, max_label);
            let message = truncate_to(&entry.message, 20);
            let time = feed_time_ago(entry);

            Line::from(vec![
                Span::styled(format!("{} ", node), node_style),
                Span::styled(
                    format!("{:<width$} ", label, width = max_label),
                    label_style,
                ),
                Span::styled(format!("{:<20} ", message), msg_style),
                Span::styled(time, time_style),
            ])
            .into()
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}

/// Returns 0.0 (old) to 1.0 (brand new), fading over GLOW_SECS.
fn recency(entry: &ActivityEntry) -> f32 {
    const GLOW_SECS: f32 = 1800.0; // 30 minutes
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let elapsed = now.saturating_sub(entry.received_at_secs) as f32;
    (1.0 - elapsed / GLOW_SECS).clamp(0.0, 1.0)
}

/// Base color for each activity kind.
fn kind_base_color(kind: &ActivityKind) -> Color {
    match kind {
        ActivityKind::AgentBusy => ACCENT,
        ActivityKind::AgentIdle => Color::Rgb(100, 220, 100),
        ActivityKind::AgentRetry => Color::Rgb(220, 200, 60),
        ActivityKind::MrPipelineFailed => Color::Rgb(220, 80, 80),
        ActivityKind::MrPipelineSucceeded => Color::Rgb(100, 220, 100),
        ActivityKind::MrNewDiscussions => Color::Rgb(80, 200, 220),
        ActivityKind::MrApproved => Color::Rgb(100, 220, 100),
    }
}

/// Node character and style based on recency.
fn node_for_recency(r: f32, base: Color) -> (&'static str, Style) {
    if r > 0.7 {
        // Brand new: filled circle, bold, bright base color
        ("◉", Style::default().fg(base).add_modifier(Modifier::BOLD))
    } else if r > 0.1 {
        // Recent: filled circle, normal base color
        ("●", Style::default().fg(base))
    } else {
        // Old: open circle, dark gray
        ("○", Style::default().fg(Color::Indexed(238)))
    }
}

/// Text styles for label, message, and time based on recency.
fn text_styles_for_recency(r: f32, base: Color) -> (Style, Style, Style) {
    if r > 0.7 {
        // Very new: bold, bright
        (
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(base).add_modifier(Modifier::BOLD),
            Style::default()
                .fg(Color::Indexed(242))
                .add_modifier(Modifier::BOLD),
        )
    } else if r > 0.1 {
        // Recent: normal colors
        (
            Style::default().fg(Color::Gray),
            Style::default().fg(base),
            Style::default().fg(Color::Indexed(238)),
        )
    } else {
        // Old: all dark gray
        let dim = Style::default().fg(Color::Indexed(237));
        (dim, dim, dim)
    }
}

/// Format elapsed time since an activity entry was recorded by the daemon.
fn feed_time_ago(entry: &ActivityEntry) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now.saturating_sub(entry.received_at_secs);
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
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
