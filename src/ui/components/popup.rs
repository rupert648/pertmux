use crate::app::PopupState;
use crate::client::ClientState;
use crate::config::{AgentActionConfig, ProjectForge};
use crate::ui::ACCENT;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
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

    if let PopupState::AgentActions { selected, .. } = &state.popup {
        draw_agent_actions_popup(frame, &state.snapshot.agent_actions, *selected, area);
        return;
    }

    if let PopupState::MrOverview { selected } = &state.popup {
        draw_mr_overview_popup(frame, state, *selected, area);
        return;
    }

    let (title, body_lines, show_cursor) = match &state.popup {
        PopupState::None
        | PopupState::ProjectFilter { .. }
        | PopupState::ChangeSummary { .. }
        | PopupState::AgentActions { .. }
        | PopupState::MrOverview { .. } => {
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

fn draw_agent_actions_popup(
    frame: &mut Frame,
    actions: &[AgentActionConfig],
    selected: usize,
    area: Rect,
) {
    let popup_w = 42u16.min(area.width.saturating_sub(4));
    let popup_h = (actions.len() as u16 + 4).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let rect = Rect::new(x, y, popup_w, popup_h);

    let block = Block::default()
        .title(Line::from(Span::styled(
            " Agent Actions ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    let inner = block.inner(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

    let mut action_lines: Vec<Line> = Vec::new();
    for (i, action) in actions.iter().enumerate() {
        let style = if i == selected {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let prefix = if i == selected { " \u{25b8} " } else { "   " };
        action_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(ACCENT)),
            Span::styled(action.name.as_str(), style),
        ]));
    }

    frame.render_widget(Paragraph::new(action_lines), chunks[0]);

    let hint = Line::from(Span::styled(
        "j/k navigate \u{00b7} Enter select \u{00b7} Esc cancel",
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(hint), chunks[1]);
}

pub(crate) fn draw_mr_overview_popup(
    frame: &mut Frame,
    state: &ClientState,
    selected: usize,
    area: Rect,
) {
    let entries = &state.snapshot.global_mrs;
    if entries.is_empty() {
        return;
    }

    let max_visible = 15usize;
    let visible_count = entries.len().min(max_visible);
    let popup_w = (area.width * 3 / 4)
        .max(60)
        .min(area.width.saturating_sub(4));
    let popup_h = (visible_count as u16 + 5).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let rect = Rect::new(x, y, popup_w, popup_h);

    let block = Block::default()
        .title(Line::from(Span::styled(
            " My Open MRs ",
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

    let available_width = inner.width as usize;
    let mut item_lines: Vec<Line> = Vec::new();

    for (i, entry) in entries
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_count)
    {
        let is_selected = i == selected;
        let prefix = if is_selected { " \u{25b8} " } else { "   " };

        let forge_badge = match entry.forge {
            ProjectForge::Gitlab => "[GL]",
            ProjectForge::Github => "[GH]",
        };

        let iid_prefix = match entry.forge {
            ProjectForge::Gitlab => "!",
            ProjectForge::Github => "#",
        };

        let linked_badge = if entry.configured_project.is_some() {
            " [linked]"
        } else {
            ""
        };

        let age = format_entry_age(&entry.mr.updated_at);

        let (text_style, badge_style) = if is_selected {
            (
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )
        } else if entry.mr.draft {
            (
                Style::default().fg(Color::DarkGray),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            (
                Style::default().fg(Color::White),
                Style::default().fg(ACCENT),
            )
        };

        let age_style = Style::default().fg(Color::DarkGray);

        let meta = format!(
            "{} {} {}{} ",
            forge_badge, entry.mr.project_path, iid_prefix, entry.mr.iid
        );
        let suffix = format!("{}  {}", linked_badge, age);
        let meta_and_suffix_len = meta.len() + suffix.len() + 3;
        let title_max = available_width.saturating_sub(meta_and_suffix_len);
        let title = crate::ui::helpers::truncate(&entry.mr.title, title_max);

        item_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(ACCENT)),
            Span::styled(meta, text_style),
            Span::styled(title, text_style),
            Span::styled(linked_badge, badge_style),
            Span::styled(format!("  {}", age), age_style),
        ]));
    }

    if item_lines.is_empty() {
        item_lines.push(Line::from(Span::styled(
            "   no open MRs",
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
        "Enter open/go to \u{00b7} Esc close",
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(help), chunks[2]);
}

fn format_entry_age(ts: &jiff::Timestamp) -> String {
    let now = jiff::Timestamp::now();
    let secs = (now - *ts).get_seconds().abs();
    if secs < 60 {
        return "just now".to_string();
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{}m ago", mins);
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{}h ago", hours);
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{}d ago", days);
    }
    let months = days / 30;
    format!("{}mo ago", months)
}
