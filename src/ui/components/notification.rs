use crate::client::ClientState;
use crate::ui::helpers::truncate;
use crate::ui::{ACCENT, NOTIFICATION_DURATION};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

pub(crate) fn draw_notification_client(frame: &mut Frame, state: &ClientState, area: Rect) {
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
