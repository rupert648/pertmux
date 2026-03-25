use crate::client::ClientState;
use crate::ui::helpers::truncate;
use crate::ui::{ACCENT, NOTIFICATION_DURATION};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

/// Render the notification overlay.
///
/// When the daemon is mid-refresh it sends `DaemonMsg::Progress` updates which
/// are stored in `state.refresh_steps`.  While those are present we display a
/// multi-line progress box instead of (or in addition to) the regular toast.
/// Once the daemon clears the steps (`Progress(vec![])`) we fall back to the
/// plain toast if one is still within its expiry window.
pub(crate) fn draw_notification_client(frame: &mut Frame, state: &ClientState, area: Rect) {
    if !state.refresh_steps.is_empty() {
        draw_progress(frame, state, area);
    } else {
        draw_toast(frame, state, area);
    }
}

fn draw_progress(frame: &mut Frame, state: &ClientState, area: Rect) {
    let steps = &state.refresh_steps;

    // Build content lines: header + one line per step.
    let mut lines: Vec<Line> = Vec::with_capacity(steps.len() + 1);

    lines.push(Line::from(Span::styled(
        "Refreshing…",
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    )));

    for step in steps {
        let bar = progress_bar(step.done, step.total, 8);
        let count = format!("[{}/{}]", step.done, step.total);
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<16}", truncate(&step.label, 16)),
                Style::default().fg(Color::White),
            ),
            Span::styled(bar, Style::default().fg(ACCENT)),
            Span::styled(format!(" {}", count), Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Compute box dimensions.
    let content_width = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.len()).sum::<usize>())
        .max()
        .unwrap_or(12) as u16;
    let width = (content_width + 4)
        .max(20)
        .min(area.width.saturating_sub(4));
    let height = (lines.len() as u16) + 2; // +2 for borders

    let x = area.width.saturating_sub(width).saturating_sub(2);
    let y = area.height.saturating_sub(height + 1);
    let rect = Rect::new(x, y, width, height);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    let para = Paragraph::new(lines).block(block);

    frame.render_widget(Clear, rect);
    frame.render_widget(para, rect);
}

fn draw_toast(frame: &mut Frame, state: &ClientState, area: Rect) {
    let Some((ref msg, at)) = state.notification else {
        return;
    };
    if at.elapsed() > NOTIFICATION_DURATION {
        return;
    }

    let width = (msg.len() as u16 + 4).min(area.width.saturating_sub(4));
    let x = area.width.saturating_sub(width).saturating_sub(2);
    let y = area.height.saturating_sub(3);
    let rect = Rect::new(x, y, width, 3);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    let text = Paragraph::new(Line::from(Span::styled(
        truncate(msg, width.saturating_sub(2) as usize),
        Style::default().fg(Color::White),
    )))
    .block(block);

    frame.render_widget(Clear, rect);
    frame.render_widget(text, rect);
}

/// Return a compact ASCII progress bar of `width` characters.
/// e.g. `[████░░░░]` (but without the outer brackets, which are added by the caller)
fn progress_bar(done: usize, total: usize, width: usize) -> String {
    if total == 0 || width == 0 {
        return " ".repeat(width);
    }
    let filled = (done * width / total).min(width);
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}
