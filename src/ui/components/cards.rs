use crate::linking::LinkedMergeRequest;
use crate::types::AgentPane;
use crate::ui::ACCENT;
use crate::ui::helpers::{compact_status_badge, merge_status_display, truncate};
use crate::worktrunk::{self, WtWorktree};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub(crate) fn render_mr_card(
    frame: &mut Frame,
    linked: &LinkedMergeRequest,
    rect: Rect,
    is_selected: bool,
) {
    let border_color = if is_selected {
        ACCENT
    } else {
        Color::Indexed(238)
    };

    let iid_label = format!(" !{} ", linked.mr.iid);
    let iid_style = if is_selected {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(Line::from(Span::styled(iid_label, iid_style)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let card_inner = block.inner(rect);
    frame.render_widget(block, rect);

    if card_inner.width == 0 || card_inner.height == 0 {
        return;
    }

    let title_color = if is_selected {
        Color::White
    } else {
        Color::Gray
    };
    let content_w = card_inner.width as usize;
    let draft_space = if linked.mr.draft { 9 } else { 0 };
    let title = truncate(&linked.mr.title, content_w.saturating_sub(draft_space));

    let mut title_spans = vec![Span::styled(title, Style::default().fg(title_color))];
    if linked.mr.draft {
        title_spans.push(Span::styled(
            " [draft]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::DIM),
        ));
    }
    if is_selected {
        for s in &mut title_spans {
            s.style = s.style.add_modifier(Modifier::BOLD);
        }
    }

    let (icon, text, color) = merge_status_display(
        linked.mr.detailed_merge_status.as_deref(),
        linked.mr.has_conflicts,
    );
    let mut status_spans: Vec<Span> = vec![
        Span::styled(format!("{} {}", icon, text), Style::default().fg(color)),
        Span::styled(" \u{00b7} ", Style::default().fg(Color::Indexed(238))),
        Span::styled(
            format!("{} comments", linked.mr.user_notes_count),
            Style::default().fg(Color::DarkGray),
        ),
    ];
    if linked.has_new_activity {
        status_spans.push(Span::styled(
            " \u{00b7} ",
            Style::default().fg(Color::Indexed(238)),
        ));
        status_spans.push(Span::styled(
            "\u{25cf} new",
            Style::default().fg(Color::Yellow),
        ));
    }
    if let Some(ref pane) = linked.tmux_pane {
        status_spans.push(Span::raw(" "));
        status_spans.push(compact_status_badge(&pane.status));
    }

    let content = vec![Line::from(title_spans), Line::from(status_spans)];
    frame.render_widget(Paragraph::new(content), card_inner);
}

pub(crate) fn render_worktree_card(
    frame: &mut Frame,
    wt: &WtWorktree,
    pane: Option<&AgentPane>,
    rect: Rect,
    is_selected: bool,
) {
    let border_color = if is_selected {
        ACCENT
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

    let mut line1_spans: Vec<Span> = Vec::new();
    if let Some(ref symbols) = wt.symbols
        && !symbols.is_empty()
    {
        line1_spans.push(Span::styled(
            format!("{} ", symbols),
            Style::default().fg(Color::Yellow),
        ));
    }
    let msg = &wt.commit.message;
    let truncated = if msg.len() > card_inner.width as usize - 4 {
        format!("{}\u{2026}", &msg[..card_inner.width as usize - 5])
    } else {
        msg.clone()
    };
    line1_spans.push(Span::styled(truncated, Style::default().fg(Color::Gray)));

    let age = worktrunk::format_age(wt.commit.timestamp);
    let mut line2_spans = vec![Span::styled(age, Style::default().fg(Color::DarkGray))];
    if let Some(pane) = pane {
        line2_spans.push(Span::raw(" "));
        line2_spans.push(compact_status_badge(&pane.status));
    }

    let content = vec![Line::from(line1_spans), Line::from(line2_spans)];
    frame.render_widget(Paragraph::new(content), card_inner);
}
