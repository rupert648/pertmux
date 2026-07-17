use crate::types::AgentPane;
use crate::ui::ACCENT;
use crate::ui::helpers::{compact_status_badge, truncate};
use crate::worktrunk::{self, WtWorktree};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub(crate) fn render_worktree_card(
    frame: &mut Frame,
    wt: &WtWorktree,
    pane: Option<&AgentPane>,
    rect: Rect,
    is_selected: bool,
) {
    let border_color = if is_selected {
        ACCENT
    } else if let Some(pane) = pane
        && let Some(changed_at) = pane.status_changed_at
    {
        // Glow orange for up to 30s after a status change
        let now = jiff::Timestamp::now();
        let elapsed_secs = (now.as_second() - changed_at.as_second()).max(0) as f32;
        let recency = (1.0 - elapsed_secs / 30.0).clamp(0.0, 1.0_f32);
        if recency > 0.0 {
            // Lerp from ACCENT (255,140,0) toward dark gray (56,56,56) based on age
            let t = 1.0 - recency; // t=0 means brand new (full ACCENT), t=1 means old (dark)
            let r = (255.0 * (1.0 - t) + 56.0 * t) as u8;
            let g = (140.0 * (1.0 - t) + 56.0 * t) as u8;
            let b = (0.0_f32 * (1.0 - t) + 56.0 * t) as u8;
            Color::Rgb(r, g, b)
        } else {
            Color::Indexed(238)
        }
    } else {
        Color::Indexed(238)
    };

    let branch = wt.branch.as_deref().unwrap_or("(detached)");
    let label_style = if is_selected {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else if wt.is_main {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Gray)
    };

    let mut title_spans = vec![Span::styled(format!(" {} ", branch), label_style)];

    if let Some(ref main) = wt.main {
        if main.ahead > 0 {
            title_spans.push(Span::styled(
                format!("\u{2191}{}", main.ahead),
                Style::default().fg(Color::Green),
            ));
            title_spans.push(Span::raw(" "));
        }
        if main.behind > 0 {
            title_spans.push(Span::styled(
                format!("\u{2193}{}", main.behind),
                Style::default().fg(Color::Red),
            ));
            title_spans.push(Span::raw(" "));
        }
    }

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let card_inner = block.inner(rect);
    frame.render_widget(block, rect);

    if card_inner.width == 0 || card_inner.height == 0 {
        return;
    }

    let age = worktrunk::format_age(wt.commit.timestamp);
    let symbols = wt
        .symbols
        .as_deref()
        .filter(|symbols| !symbols.is_empty())
        .map(|symbols| format!("{} ", symbols))
        .unwrap_or_default();
    let metadata_width = age.chars().count() + 2 + usize::from(pane.is_some()) * 4;
    let message_width =
        (card_inner.width as usize).saturating_sub(symbols.chars().count() + metadata_width);
    let message = truncate(&wt.commit.message, message_width);
    let used_width = symbols.chars().count() + message.chars().count() + metadata_width;
    let padding = (card_inner.width as usize).saturating_sub(used_width);

    let mut content_spans = Vec::new();
    if !symbols.is_empty() {
        content_spans.push(Span::styled(symbols, Style::default().fg(Color::Yellow)));
    }
    content_spans.push(Span::styled(message, Style::default().fg(Color::Gray)));
    content_spans.push(Span::raw(" ".repeat(padding + 2)));
    content_spans.push(Span::styled(age, Style::default().fg(Color::DarkGray)));
    if let Some(pane) = pane {
        content_spans.push(Span::raw(" "));
        content_spans.push(compact_status_badge(&pane.status));
    }

    frame.render_widget(Paragraph::new(Line::from(content_spans)), card_inner);
}
