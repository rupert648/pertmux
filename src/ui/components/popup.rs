use crate::app::PopupState;
use crate::client::ClientState;
use crate::config::{AgentActionConfig, ProjectForge};
use crate::protocol::ActivityKind;
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

    if let PopupState::AgentActions { selected, .. } = &state.popup {
        draw_agent_actions_popup(frame, &state.snapshot.agent_actions, *selected, area);
        return;
    }

    if let PopupState::MrOverview { selected } = &state.popup {
        draw_mr_overview_popup(frame, state, *selected, area);
        return;
    }

    if let PopupState::ActivityFeed { selected } = &state.popup {
        draw_activity_feed_popup(frame, state, *selected, area);
        return;
    }

    if matches!(state.popup, PopupState::KeybindingsHelp) {
        draw_keybindings_popup(frame, state, area);
        return;
    }

    if let PopupState::CreateWorktreeWithPrompt {
        branch_input,
        prompt_input,
        focused_field,
    } = &state.popup
    {
        draw_create_with_prompt_popup(
            frame,
            branch_input,
            prompt_input,
            *focused_field,
            state,
            area,
        );
        return;
    }

    let (title, body_lines, show_cursor) = match &state.popup {
        PopupState::None
        | PopupState::ProjectFilter { .. }
        | PopupState::ChangeSummary { .. }
        | PopupState::AgentActions { .. }
        | PopupState::MrOverview { .. }
        | PopupState::ActivityFeed { .. }
        | PopupState::KeybindingsHelp
        | PopupState::CreateWorktreeWithPrompt { .. } => {
            return;
        }
        PopupState::ConfirmKillTmuxWindow { branch, .. } => {
            let lines = vec![
                Line::from(vec![
                    Span::styled("Kill tmux window for ", Style::default().fg(Color::Gray)),
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
                    "The linked tmux window will be closed.",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Enter confirm \u{00b7} Esc skip",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            (" Kill Tmux Window ", lines, false)
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
        PopupState::ConfirmRemove { branch, .. } => {
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

    // Build render items: project-group headers interleaved with entry indices.
    // entries are pre-sorted by project_path in the daemon, so groups are contiguous.
    enum RenderItem {
        Header {
            project: String,
            forge: ProjectForge,
        },
        Entry(usize), // index into entries
    }

    let mut render_items: Vec<RenderItem> = Vec::new();
    let mut last_project = "";
    for (idx, entry) in entries.iter().enumerate() {
        if entry.mr.project_path.as_str() != last_project {
            render_items.push(RenderItem::Header {
                project: entry.mr.project_path.clone(),
                forge: entry.forge.clone(),
            });
            last_project = entry.mr.project_path.as_str();
        }
        render_items.push(RenderItem::Entry(idx));
    }

    // Find the visual (render) line of the selected entry so we can scroll correctly.
    let selected_visual = render_items
        .iter()
        .position(|item| matches!(item, RenderItem::Entry(i) if *i == selected))
        .unwrap_or(0);

    let max_visible = 15usize;
    let total_lines = render_items.len();
    let visible_count = total_lines.min(max_visible);

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

    // Scroll so the selected entry's visual line stays within the visible window.
    let scroll_offset = if selected_visual >= visible_count {
        selected_visual - visible_count + 1
    } else {
        0
    };

    let available_width = inner.width as usize;
    let mut item_lines: Vec<Line> = Vec::new();

    for item in render_items.iter().skip(scroll_offset).take(visible_count) {
        match item {
            RenderItem::Header { project, forge } => {
                let badge = match forge {
                    ProjectForge::Gitlab => "[GL]",
                    ProjectForge::Github => "[GH]",
                };
                item_lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {} ", badge),
                        Style::default()
                            .fg(Color::Indexed(244))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        project.as_str(),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            RenderItem::Entry(idx) => {
                let entry = &entries[*idx];
                let is_selected = *idx == selected;
                let prefix = if is_selected { " \u{25b8} " } else { "   " };

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
                let iid_str = format!("{}{}", iid_prefix, entry.mr.iid);

                let (title_style, iid_style, badge_style) = if is_selected {
                    (
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    )
                } else if entry.mr.draft {
                    (
                        Style::default().fg(Color::DarkGray),
                        Style::default().fg(Color::DarkGray),
                        Style::default().fg(Color::DarkGray),
                    )
                } else {
                    (
                        Style::default().fg(Color::White),
                        Style::default().fg(Color::Indexed(242)),
                        Style::default().fg(ACCENT),
                    )
                };

                let age_style = Style::default().fg(Color::DarkGray);

                // prefix(3) + iid + space(1) + title + badge + "  " + age
                let fixed_len = 3 + iid_str.len() + 1 + linked_badge.len() + 2 + age.len();
                let title_max = available_width.saturating_sub(fixed_len);
                let title = crate::ui::helpers::truncate(&entry.mr.title, title_max);

                item_lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(ACCENT)),
                    Span::styled(format!("{} ", iid_str), iid_style),
                    Span::styled(title, title_style),
                    Span::styled(linked_badge, badge_style),
                    Span::styled(format!("  {}", age), age_style),
                ]));
            }
        }
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

fn draw_activity_feed_popup(frame: &mut Frame, state: &ClientState, selected: usize, area: Rect) {
    let entries = &state.snapshot.activity_feed;
    if entries.is_empty() {
        return;
    }

    let max_visible = 15usize;
    let visible_count = entries.len().min(max_visible);
    let popup_w = (area.width * 3 / 4)
        .max(60)
        .min(area.width.saturating_sub(4));
    let popup_h = (visible_count as u16 + 4).min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let rect = Rect::new(x, y, popup_w, popup_h);

    let block = Block::default()
        .title(Line::from(Span::styled(
            " Activity Feed ",
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
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut item_lines: Vec<Line> = Vec::new();

    for (i, entry) in entries
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_count)
    {
        let is_selected = i == selected;
        let prefix = if is_selected { " \u{25b8} " } else { "   " };

        let base_color = activity_kind_color(&entry.kind);

        let (name_style, msg_style, time_style) = if is_selected {
            (
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                Style::default().fg(base_color).add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                Style::default().fg(Color::White),
                Style::default().fg(base_color),
                Style::default().fg(Color::DarkGray),
            )
        };

        let elapsed = now_secs.saturating_sub(entry.received_at_secs);
        let time = if elapsed < 60 {
            format!("{}s", elapsed)
        } else if elapsed < 3600 {
            format!("{}m", elapsed / 60)
        } else {
            format!("{}h", elapsed / 3600)
        };

        // Reserve: 3 (prefix) + 21 (message 20+space) + time.len() + 1 (space) = ~30 chars min.
        let fixed = 3 + 21 + time.len() + 1;
        let label_max = available_width.saturating_sub(fixed).max(8);
        let label = crate::ui::helpers::truncate(&entry.label, label_max);

        item_lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(ACCENT)),
            Span::styled(format!("{:<width$} ", label, width = label_max), name_style),
            Span::styled(format!("{:<20} ", entry.message), msg_style),
            Span::styled(time, time_style),
        ]));
    }

    if item_lines.is_empty() {
        item_lines.push(Line::from(Span::styled(
            "   no activity yet",
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
        "j/k navigate \u{00b7} Enter go to \u{00b7} Esc close",
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(help), chunks[2]);
}

/// Base color for each activity kind (used in the popup).
fn activity_kind_color(kind: &ActivityKind) -> Color {
    match kind {
        ActivityKind::AgentBusy => ACCENT,
        ActivityKind::AgentIdle => Color::Rgb(100, 220, 100),
        ActivityKind::AgentRetry => Color::Rgb(220, 200, 60),
        ActivityKind::MrPipelineFailed => Color::Rgb(220, 80, 80),
        ActivityKind::MrPipelineSucceeded => Color::Rgb(100, 220, 100),
        ActivityKind::MrNewDiscussions => Color::Rgb(80, 200, 220),
        ActivityKind::MrApproved => Color::Rgb(100, 220, 100),
    }
}

fn draw_create_with_prompt_popup(
    frame: &mut Frame,
    branch_input: &str,
    prompt_input: &str,
    focused_field: usize,
    state: &ClientState,
    area: Rect,
) {
    // Show the template hint so users know what the command will look like.
    let template_hint = state
        .snapshot
        .default_worktree_with_prompt
        .as_deref()
        .unwrap_or("");

    let popup_w = 64u16.min(area.width.saturating_sub(4));
    // 2 border + title/hint line + blank + branch label + branch input + blank
    // + prompt label + prompt input + blank + hint = ~12 inner rows + 2 border
    let popup_h = 14u16.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let rect = Rect::new(x, y, popup_w, popup_h);

    let block = Block::default()
        .title(Line::from(Span::styled(
            " Create Worktree with Prompt ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    let inner = block.inner(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);

    // Layout: template hint, blank, branch field, blank, prompt field, blank, help
    let chunks = Layout::vertical([
        Constraint::Length(1), // template hint
        Constraint::Length(1), // blank
        Constraint::Length(1), // branch label
        Constraint::Length(1), // branch input
        Constraint::Length(1), // blank
        Constraint::Length(1), // prompt label
        Constraint::Length(1), // prompt input
        Constraint::Min(1),    // blank / overflow
        Constraint::Length(1), // help line
    ])
    .split(inner);

    // Template hint
    let hint_text = if template_hint.is_empty() {
        "no template configured".to_string()
    } else {
        format!("template: {}", template_hint)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint_text,
            Style::default().fg(Color::DarkGray),
        ))),
        chunks[0],
    );

    // Branch label
    let branch_label_style = if focused_field == 0 {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("Branch name:", branch_label_style))),
        chunks[2],
    );

    // Branch input
    let branch_line = if focused_field == 0 {
        Line::from(vec![
            Span::styled(
                format!(" {}", branch_input),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("\u{2588}", Style::default().fg(ACCENT)),
        ])
    } else {
        Line::from(Span::styled(
            format!(" {}", branch_input),
            Style::default().fg(Color::White),
        ))
    };
    frame.render_widget(Paragraph::new(branch_line), chunks[3]);

    // Prompt label
    let prompt_label_style = if focused_field == 1 {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("Message:", prompt_label_style))),
        chunks[5],
    );

    // Prompt input
    let prompt_line = if focused_field == 1 {
        Line::from(vec![
            Span::styled(
                format!(" {}", prompt_input),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("\u{2588}", Style::default().fg(ACCENT)),
        ])
    } else {
        Line::from(Span::styled(
            format!(" {}", prompt_input),
            Style::default().fg(Color::White),
        ))
    };
    frame.render_widget(Paragraph::new(prompt_line), chunks[6]);

    // Help line
    let help = Line::from(Span::styled(
        "Tab switch field \u{00b7} Enter confirm \u{00b7} Esc cancel",
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(help), chunks[8]);
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

fn draw_keybindings_popup(frame: &mut Frame, state: &ClientState, area: Rect) {
    // Static navigation entries (not configurable).
    let nav_entries: &[(&str, &str)] = &[
        ("\u{2191}\u{2193} / jk", "Navigate"),
        ("Tab", "Switch section (MRs / Worktrees)"),
        ("\u{23ce}", "Focus pane in tmux"),
        ("q / Esc", "Quit"),
    ];

    // Configurable entries sourced from KeybindingsConfig::entries().
    // Adding a new field to KeybindingsConfig and its entry there automatically
    // surfaces it here — no changes to this function required.
    let kb = &state.snapshot.keybindings;
    let config_entries = kb.entries();

    // Extra hardcoded action entries that are not in KeybindingsConfig.
    let extra_entries: &[(char, &str)] = &[('K', "Keybindings (this modal)")];

    // Total rows: header + nav + blank + header + config + extra + blank + help.
    let content_rows = 1 + nav_entries.len() + 1 + 1 + config_entries.len() + extra_entries.len();
    let popup_h = (content_rows as u16 + 4).min(area.height.saturating_sub(4));
    let popup_w = 52u16.min(area.width.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let rect = Rect::new(x, y, popup_w, popup_h);

    let block = Block::default()
        .title(Line::from(Span::styled(
            " Keybindings ",
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

    let key_col = 14usize;
    let mut lines: Vec<Line> = Vec::new();

    // Navigation section.
    lines.push(Line::from(Span::styled(
        "Navigation",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )));
    for (key, desc) in nav_entries {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<width$}", key, width = key_col),
                Style::default().fg(ACCENT),
            ),
            Span::styled(*desc, Style::default().fg(Color::White)),
        ]));
    }

    lines.push(Line::from(""));

    // Actions section (configurable + fixed extras).
    lines.push(Line::from(Span::styled(
        "Actions",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )));
    for (key, desc) in &config_entries {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<width$}", key, width = key_col),
                Style::default().fg(ACCENT),
            ),
            Span::styled(*desc, Style::default().fg(Color::White)),
        ]));
    }
    for (key, desc) in extra_entries {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<width$}", key, width = key_col),
                Style::default().fg(ACCENT),
            ),
            Span::styled(*desc, Style::default().fg(Color::White)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), chunks[0]);

    let divider = Line::from(Span::styled(
        "\u{2500}".repeat(inner.width as usize),
        Style::default().fg(Color::Indexed(236)),
    ));
    frame.render_widget(Paragraph::new(divider), chunks[1]);

    let help = Line::from(Span::styled(
        "Esc close",
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(help), chunks[2]);
}
