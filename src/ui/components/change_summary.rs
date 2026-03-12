use crate::mr_changes::{MrChange, MrChangeType};
use crate::ui::ACCENT;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

pub(crate) fn draw_change_summary(
    frame: &mut Frame,
    changes: &[MrChange],
    selected: usize,
    area: Rect,
) {
    if changes.is_empty() {
        return;
    }

    let max_visible = 12usize;
    let visible_count = changes.len().min(max_visible);
    let popup_w = 60u16.min(area.width.saturating_sub(4));
    let popup_h = (visible_count as u16 + 4).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let rect = Rect::new(x, y, popup_w, popup_h);

    let block = Block::default()
        .title(Line::from(Span::styled(
            " Changes While Away ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    let inner = block.inner(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);

    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let scroll_offset = if selected >= visible_count {
        selected - visible_count + 1
    } else {
        0
    };

    let mut item_lines: Vec<Line> = Vec::new();
    for (i, change) in changes
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_count)
    {
        let is_selected = i == selected;
        let prefix = if is_selected { " \u{25b8} " } else { "   " };

        let change_color = match &change.change_type {
            MrChangeType::PipelineFailed => Color::Red,
            MrChangeType::PipelineSucceeded => Color::Green,
            MrChangeType::NewDiscussions(_) => Color::Cyan,
            MrChangeType::Approved => Color::Green,
        };

        let action = match &change.change_type {
            MrChangeType::PipelineFailed => "Pipeline failed".to_string(),
            MrChangeType::PipelineSucceeded => "Pipeline succeeded".to_string(),
            MrChangeType::NewDiscussions(n) => {
                if *n == 1 {
                    "1 new discussion".to_string()
                } else {
                    format!("{} new discussions", n)
                }
            }
            MrChangeType::Approved => "Approved".to_string(),
        };

        let name_style = if is_selected {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let action_style = if is_selected {
            Style::default()
                .fg(change_color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(change_color)
        };

        item_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(ACCENT)),
            Span::styled(
                format!("{} !{}: ", change.project_name, change.mr_iid),
                name_style,
            ),
            Span::styled(action, action_style),
        ]));
    }

    if item_lines.is_empty() {
        item_lines.push(Line::from(Span::styled(
            "   no changes",
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(Paragraph::new(item_lines), chunks[0]);

    let divider = Line::from(Span::styled(
        "\u{2500}".repeat(inner.width as usize),
        Style::default().fg(Color::Indexed(236)),
    ));
    frame.render_widget(Paragraph::new(divider), chunks[1]);

    let help = Line::from(Span::styled(
        "Enter go to MR \u{00b7} Esc dismiss",
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(help), chunks[2]);
}
