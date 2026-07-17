use crate::ui::ACCENT;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

const SPINNER_FRAMES: [&str; 4] = ["\u{25d0}", "\u{25d3}", "\u{25d1}", "\u{25d2}"];

pub(crate) fn draw_loading(frame: &mut Frame, tick: usize) {
    let area = frame.area();
    let vertical = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(5),
        Constraint::Fill(1),
    ])
    .split(area);
    let horizontal = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(32.min(area.width)),
        Constraint::Fill(1),
    ])
    .split(vertical[1]);

    let spinner = SPINNER_FRAMES[tick % SPINNER_FRAMES.len()];
    let content = vec![
        Line::from(Span::styled(
            spinner,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "pert",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "mux",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "loading dashboard...",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    frame.render_widget(
        Paragraph::new(content).alignment(Alignment::Center),
        horizontal[1],
    );
}
