use crate::app::PopupState;
use crate::client::ClientState;
use crate::ui::ACCENT;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

pub(crate) fn draw_popup_client(frame: &mut Frame, state: &ClientState, area: Rect) {
    if let PopupState::ChangeSummary { changes, selected } = &state.popup {
        super::change_summary::draw_change_summary(frame, changes, *selected, area);
        return;
    }

    if let PopupState::ProjectFilter {
        input,
        filtered,
        selected,
    } = &state.popup
    {
        draw_project_filter_popup(frame, input, filtered, *selected, area);
        return;
    }

    let (title, body_lines, show_cursor) = match &state.popup {
        PopupState::None | PopupState::ProjectFilter { .. } | PopupState::ChangeSummary { .. } => {
            return;
        }
        PopupState::CreateWorktree { input } => {
            let lines = vec![
                Line::from(Span::styled(
                    "Branch name:",
                    Style::default().fg(Color::Gray),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        format!(" {}", input),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("\u{2588}", Style::default().fg(ACCENT)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Enter confirm \u{00b7} Esc cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            (" Create Worktree ", lines, true)
        }
        PopupState::ConfirmRemove { branch } => {
            let lines = vec![
                Line::from(vec![
                    Span::styled("Remove worktree ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        branch.as_str(),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("?", Style::default().fg(Color::Gray)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Branch will be deleted if merged.",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Enter confirm \u{00b7} Esc cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            (" Remove Worktree ", lines, false)
        }
        PopupState::ConfirmMerge { branch, .. } => {
            let lines = vec![
                Line::from(vec![
                    Span::styled("Merge ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        branch.as_str(),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" into default branch?", Style::default().fg(Color::Gray)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Squash + rebase, then remove worktree.",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Enter confirm \u{00b7} Esc cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            (" Merge Worktree ", lines, false)
        }
    };

    let popup_w = 50u16.min(area.width.saturating_sub(4));
    let popup_h = (body_lines.len() as u16 + 2).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let rect = Rect::new(x, y, popup_w, popup_h);

    let block = Block::default()
        .title(Line::from(Span::styled(
            title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    let paragraph = Paragraph::new(body_lines).block(block);
    frame.render_widget(Clear, rect);
    frame.render_widget(paragraph, rect);

    let _ = show_cursor;
}

fn draw_project_filter_popup(
    frame: &mut Frame,
    input: &str,
    filtered: &[(usize, String)],
    selected: usize,
    area: Rect,
) {
    let popup_w = 50u16.min(area.width.saturating_sub(4));
    let list_h = filtered.len().min(10) as u16;
    let popup_h = (list_h + 4).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let rect = Rect::new(x, y, popup_w, popup_h);

    let block = Block::default()
        .title(Line::from(Span::styled(
            " Find Project ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    let inner = block.inner(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(inner);

    let input_line = Line::from(vec![
        Span::styled(
            " > ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            input,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("\u{2588}", Style::default().fg(ACCENT)),
    ]);
    frame.render_widget(Paragraph::new(input_line), chunks[0]);

    let divider = Line::from(Span::styled(
        "\u{2500}".repeat(inner.width as usize),
        Style::default().fg(Color::Indexed(236)),
    ));
    frame.render_widget(Paragraph::new(divider), chunks[1]);

    let mut result_lines: Vec<Line> = Vec::new();
    for (i, (_idx, name)) in filtered.iter().enumerate() {
        let style = if i == selected {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let prefix = if i == selected { " \u{25b8} " } else { "   " };
        result_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(ACCENT)),
            Span::styled(name.as_str(), style),
        ]));
    }

    if result_lines.is_empty() {
        result_lines.push(Line::from(Span::styled(
            "   no matches",
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(Paragraph::new(result_lines), chunks[2]);
}
