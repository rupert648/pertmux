use super::mr_sections::draw_mr_sections_client;
use crate::app::SelectionSection;
use crate::client::ClientState;
use crate::types::PaneStatus;
use crate::ui::ACCENT;
use crate::ui::helpers::{compute_scroll, status_badge};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

pub(crate) fn draw_list_panel_client(frame: &mut Frame, state: &ClientState, area: Rect) {
    let title_right = if let Some(proj) = state.snapshot.projects.get(state.active_project) {
        let mr_count = proj.dashboard.linked_mrs.len();
        format!(
            " {} MR{}  {}s ago ",
            mr_count,
            if mr_count == 1 { "" } else { "s" },
            state.snapshot.seconds_since_refresh,
        )
    } else {
        format!(
            " {} pane{}  {}s ago ",
            state.snapshot.panes.len(),
            if state.snapshot.panes.len() == 1 {
                ""
            } else {
                "s"
            },
            state.snapshot.seconds_since_refresh,
        )
    };

    let in_worktrees = state
        .snapshot
        .projects
        .get(state.active_project)
        .is_some_and(|_| {
            matches!(
                state.selection_section.get(state.active_project),
                Some(SelectionSection::Worktrees)
            )
        });

    let kb = &state.snapshot.keybindings;

    let hint_bottom = if !state.snapshot.projects.is_empty() {
        let mut hints = vec![
            Span::styled(" \u{2191}\u{2193}", Style::default().fg(ACCENT)),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("jk", Style::default().fg(ACCENT)),
            Span::styled(" nav  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", Style::default().fg(ACCENT)),
            Span::styled(" switch  ", Style::default().fg(Color::DarkGray)),
            Span::styled("\u{23ce}", Style::default().fg(ACCENT)),
            Span::styled(" focus  ", Style::default().fg(Color::DarkGray)),
            Span::styled(kb.refresh.to_string(), Style::default().fg(ACCENT)),
            Span::styled(" refresh  ", Style::default().fg(Color::DarkGray)),
        ];
        if in_worktrees {
            hints.push(Span::styled(
                kb.create_worktree.to_string(),
                Style::default().fg(ACCENT),
            ));
            hints.push(Span::styled(
                " create  ",
                Style::default().fg(Color::DarkGray),
            ));
            hints.push(Span::styled(
                kb.delete_worktree.to_string(),
                Style::default().fg(ACCENT),
            ));
            hints.push(Span::styled(" del  ", Style::default().fg(Color::DarkGray)));
            hints.push(Span::styled(
                kb.merge_worktree.to_string(),
                Style::default().fg(ACCENT),
            ));
            hints.push(Span::styled(
                " merge  ",
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            hints.push(Span::styled(
                kb.open_browser.to_string(),
                Style::default().fg(ACCENT),
            ));
            hints.push(Span::styled(
                " open  ",
                Style::default().fg(Color::DarkGray),
            ));
        }
        hints.push(Span::styled(
            kb.copy_branch.to_string(),
            Style::default().fg(ACCENT),
        ));
        hints.push(Span::styled(
            " branch  ",
            Style::default().fg(Color::DarkGray),
        ));
        if state.snapshot.projects.len() > 1 {
            hints.push(Span::styled(
                kb.filter_projects.to_string(),
                Style::default().fg(ACCENT),
            ));
            hints.push(Span::styled(
                " filter  ",
                Style::default().fg(Color::DarkGray),
            ));
        }
        hints.push(Span::styled("q", Style::default().fg(ACCENT)));
        hints.push(Span::styled(" quit ", Style::default().fg(Color::DarkGray)));
        Line::from(hints)
    } else {
        Line::from(vec![
            Span::styled(" \u{2191}\u{2193}", Style::default().fg(ACCENT)),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("jk", Style::default().fg(ACCENT)),
            Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
            Span::styled("\u{23ce}", Style::default().fg(ACCENT)),
            Span::styled(" focus  ", Style::default().fg(Color::DarkGray)),
            Span::styled(kb.refresh.to_string(), Style::default().fg(ACCENT)),
            Span::styled(" refresh  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(ACCENT)),
            Span::styled(" quit ", Style::default().fg(Color::DarkGray)),
        ])
    };

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " pert",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "mux ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .title(
            Line::from(Span::styled(
                &title_right,
                Style::default().fg(Color::DarkGray),
            ))
            .right_aligned(),
        )
        .title_bottom(hint_bottom.centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::new(1, 1, 1, 0));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(ref error) = state.snapshot.error {
        let msg = Paragraph::new(Line::from(Span::styled(
            error.as_str(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        frame.render_widget(msg, inner);
        return;
    }

    if let Some(proj) = state.snapshot.projects.get(state.active_project) {
        let section = state
            .selection_section
            .get(state.active_project)
            .unwrap_or(&SelectionSection::Worktrees);
        draw_mr_sections_client(
            frame,
            proj,
            &state.snapshot.panes,
            *state.mr_selected.get(state.active_project).unwrap_or(&0),
            *state
                .worktree_selected
                .get(state.active_project)
                .unwrap_or(&0),
            section,
            inner,
        );
        return;
    }

    if state.snapshot.panes.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No agent panes found.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Make sure a coding agent is running in a tmux pane.",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    let mut flat_idx: usize = 0;

    for (session_name, pane_indices) in &state.snapshot.groups {
        lines.push(Line::from(vec![
            Span::styled("  \u{25aa} ", Style::default().fg(ACCENT)),
            Span::styled(
                session_name.as_str(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
        ]));

        for &idx in pane_indices {
            let pane = &state.snapshot.panes[idx];
            let is_selected = flat_idx == state.selected;
            flat_idx += 1;

            let cursor = if is_selected { "\u{25b8} " } else { "  " };
            let badge = status_badge(&pane.status);

            let title_color = if is_selected {
                Color::White
            } else {
                Color::Gray
            };

            let mut spans = vec![
                Span::styled(
                    format!("    {}", cursor),
                    Style::default().fg(if is_selected { ACCENT } else { Color::DarkGray }),
                ),
                badge,
                Span::raw(" "),
                Span::styled(
                    pane.display_title().to_string(),
                    Style::default().fg(title_color),
                ),
            ];

            if is_selected {
                for s in &mut spans {
                    s.style = s.style.add_modifier(Modifier::BOLD);
                }
            }

            lines.push(Line::from(spans));

            let mut detail_parts: Vec<Span> = Vec::new();
            detail_parts.push(Span::styled(
                pane.display_agent().to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            detail_parts.push(Span::styled(
                " \u{00b7} ",
                Style::default().fg(Color::Indexed(238)),
            ));
            detail_parts.push(Span::styled(
                pane.display_model().to_string(),
                Style::default().fg(Color::DarkGray),
            ));

            if (pane.status == PaneStatus::Idle || pane.status == PaneStatus::Unknown)
                && let Some(ago) = pane.time_ago()
            {
                detail_parts.push(Span::styled(
                    " \u{00b7} ",
                    Style::default().fg(Color::Indexed(238)),
                ));
                detail_parts.push(Span::styled(ago, Style::default().fg(Color::DarkGray)));
            }

            let mut detail_line = vec![Span::raw("          ")];
            detail_line.extend(detail_parts);
            lines.push(Line::from(detail_line));

            if let Some(ref response) = pane.last_response {
                let preview = response.lines().next().unwrap_or("");
                if !preview.is_empty() {
                    let max_w = area.width.saturating_sub(14) as usize;
                    let truncated = if preview.len() > max_w && max_w > 3 {
                        format!("{}...", &preview[..max_w - 3])
                    } else {
                        preview.to_string()
                    };
                    lines.push(Line::from(vec![
                        Span::styled("          \u{25b9} ", Style::default().fg(Color::Green)),
                        Span::styled(truncated, Style::default().fg(Color::Indexed(250))),
                    ]));
                }
            }
        }

        lines.push(Line::from(""));
    }

    let visible_height = inner.height as usize;
    let scroll = compute_scroll(
        &lines,
        state.selected,
        &state.snapshot.groups,
        &state.snapshot.panes,
        visible_height,
    );

    let paragraph = Paragraph::new(lines).scroll((scroll as u16, 0));
    frame.render_widget(paragraph, inner);
}
