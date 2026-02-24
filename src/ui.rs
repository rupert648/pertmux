use crate::app::App;
use crate::types::OpenCodeStatus;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
    Frame,
};

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let title_right = format!(
        " {} pane{} \u{2500} refreshed {}s ago ",
        app.panes.len(),
        if app.panes.len() == 1 { "" } else { "s" },
        app.seconds_since_refresh(),
    );

    let block = Block::default()
        .title(" pertmux ")
        .title_bottom(Line::from(" \u{2191}\u{2193}/jk navigate  \u{23ce} focus  q quit ").centered())
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .padding(Padding::new(1, 1, 1, 0));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Title right side
    let title_r = Span::styled(&title_right, Style::default().fg(Color::DarkGray));
    let title_line = Line::from(title_r).right_aligned();
    if area.width as usize > title_right.len() + 12 {
        let title_area = Rect::new(area.x, area.y, area.width, 1);
        frame.render_widget(Paragraph::new(title_line), title_area);
    }

    if let Some(ref error) = app.error {
        let msg = Paragraph::new(Line::from(Span::styled(
            error.as_str(),
            Style::default().fg(Color::Red),
        )));
        frame.render_widget(msg, inner);
        return;
    }

    if app.panes.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "No opencode panes found.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Make sure opencode is running in a tmux pane.",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    let mut flat_idx: usize = 0;

    for (session_name, pane_indices) in &app.groups {
        // Session header
        lines.push(Line::from(Span::styled(
            format!("  {}", session_name),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        for &idx in pane_indices {
            let pane = &app.panes[idx];
            let is_selected = flat_idx == app.selected;
            flat_idx += 1;

            // First line: [cursor] [icon] title
            let cursor = if is_selected { "\u{25b8} " } else { "  " };
            let (icon, icon_color) = status_style(&pane.status);

            let mut spans = vec![
                Span::styled(
                    format!("  {}", cursor),
                    Style::default().fg(if is_selected {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::styled(
                    format!("{} ", icon),
                    Style::default().fg(icon_color),
                ),
                Span::styled(
                    pane.display_title().to_string(),
                    Style::default().fg(if is_selected {
                        Color::White
                    } else {
                        Color::Gray
                    }),
                ),
            ];

            if is_selected {
                for s in &mut spans {
                    s.style = s.style.add_modifier(Modifier::BOLD);
                }
            }

            lines.push(Line::from(spans));

            // Second line: agent · model · status · time ago
            let mut detail_parts: Vec<String> = Vec::new();
            detail_parts.push(pane.display_agent().to_string());
            detail_parts.push(pane.display_model().to_string());
            detail_parts.push(pane.status.label().to_string());
            if pane.status == OpenCodeStatus::Idle || pane.status == OpenCodeStatus::Unknown {
                if let Some(ago) = pane.time_ago() {
                    detail_parts.push(ago);
                }
            }

            lines.push(Line::from(Span::styled(
                format!("        {}", detail_parts.join(" \u{00b7} ")),
                Style::default().fg(Color::DarkGray),
            )));
        }

        // Spacing between groups
        lines.push(Line::from(""));
    }

    // Handle scrolling if content exceeds terminal height
    let visible_height = inner.height as usize;
    let scroll = compute_scroll(&lines, app.selected, &app.groups, visible_height);

    let paragraph = Paragraph::new(lines).scroll((scroll as u16, 0));
    frame.render_widget(paragraph, inner);
}

fn status_style(status: &OpenCodeStatus) -> (&str, Color) {
    let icon = status.icon();
    let color = match status {
        OpenCodeStatus::Idle => Color::DarkGray,
        OpenCodeStatus::Busy => Color::Green,
        OpenCodeStatus::Retry { .. } => Color::Yellow,
        OpenCodeStatus::Unknown => Color::DarkGray,
    };
    (icon, color)
}

/// Compute scroll offset to keep the selected pane visible.
fn compute_scroll(
    lines: &[Line],
    selected: usize,
    groups: &[(String, Vec<usize>)],
    visible_height: usize,
) -> usize {
    // Find the line index where the selected pane starts
    // Each group has: 1 header line, then 2 lines per pane, then 1 blank
    let mut line_idx = 0;
    let mut flat = 0;
    for (_, pane_indices) in groups {
        line_idx += 1; // header
        for _ in pane_indices {
            if flat == selected {
                // Selected pane starts at line_idx
                if line_idx + 2 > visible_height {
                    return line_idx.saturating_sub(visible_height / 2);
                }
                return 0;
            }
            line_idx += 2; // title + detail
            flat += 1;
        }
        line_idx += 1; // spacing
    }
    if lines.len() > visible_height {
        lines.len() - visible_height
    } else {
        0
    }
}
