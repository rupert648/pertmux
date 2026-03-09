use crate::client::ClientState;
use crate::ui::ACCENT;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub(crate) fn draw_overview_panel(frame: &mut Frame, state: &ClientState, area: Rect) {
    let block = Block::default()
        .title(Line::from(Span::styled(
            " Projects ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    for (i, proj) in state.snapshot.projects.iter().enumerate() {
        let is_active = i == state.active_project;
        let mr_count = proj.dashboard.linked_mrs.len();

        let marker = if is_active { " \u{25b8} " } else { "   " };
        let name_style = if is_active {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let count_style = if mr_count > 0 {
            Style::default().fg(ACCENT)
        } else {
            Style::default().fg(Color::Indexed(238))
        };

        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(ACCENT)),
            Span::styled(&proj.name, name_style),
            Span::styled(format!("  \u{25cf} {}", mr_count), count_style),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}
