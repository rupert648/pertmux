use crate::app::{App, SelectionSection};
use crate::types::{PaneStatus, SessionDetail};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
    Frame,
};

/// Whether the terminal is landscape (wide) or portrait (tall).
fn is_landscape(area: Rect) -> bool {
    area.width >= area.height * 2
}

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    if is_landscape(area) {
        // ┌─ list ─┬─ detail ─┐
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);
        draw_list_panel(frame, app, chunks[0]);
        draw_detail_panel(frame, app, chunks[1]);
    } else {
        // ┌─ list ──┐
        // ├─ detail ─┤
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);
        draw_list_panel(frame, app, chunks[0]);
        draw_detail_panel(frame, app, chunks[1]);
    }
}

// ─── List panel ───────────────────────────────────────────────────────────────

fn draw_list_panel(frame: &mut Frame, app: &App, area: Rect) {
    // Title bar: show MR count if gitlab configured, else pane count
    let title_right = if app.gitlab_client.is_some() {
        let mr_count = app.dashboard.linked_mrs.len();
        format!(
            " {} MR{}  {}s ago ",
            mr_count,
            if mr_count == 1 { "" } else { "s" },
            app.seconds_since_refresh(),
        )
    } else {
        format!(
            " {} pane{}  {}s ago ",
            app.panes.len(),
            if app.panes.len() == 1 { "" } else { "s" },
            app.seconds_since_refresh(),
        )
    };

    // Bottom hint bar: add Tab and o for MR mode
    let hint_bottom = if app.gitlab_client.is_some() {
        Line::from(vec![
            Span::styled(" ↑↓", Style::default().fg(Color::Cyan)),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("jk", Style::default().fg(Color::Cyan)),
            Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", Style::default().fg(Color::Cyan)),
            Span::styled(" switch  ", Style::default().fg(Color::DarkGray)),
            Span::styled("⏎", Style::default().fg(Color::Cyan)),
            Span::styled(" focus  ", Style::default().fg(Color::DarkGray)),
            Span::styled("r", Style::default().fg(Color::Cyan)),
            Span::styled(" refresh  ", Style::default().fg(Color::DarkGray)),
            Span::styled("o", Style::default().fg(Color::Cyan)),
            Span::styled(" open  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::styled(" quit ", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" ↑↓", Style::default().fg(Color::Cyan)),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("jk", Style::default().fg(Color::Cyan)),
            Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
            Span::styled("⏎", Style::default().fg(Color::Cyan)),
            Span::styled(" focus  ", Style::default().fg(Color::DarkGray)),
            Span::styled("r", Style::default().fg(Color::Cyan)),
            Span::styled(" refresh  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::styled(" quit ", Style::default().fg(Color::DarkGray)),
        ])
    };

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " pert",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
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

    if let Some(ref error) = app.error {
        let msg = Paragraph::new(Line::from(Span::styled(
            error.as_str(),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        frame.render_widget(msg, inner);
        return;
    }

    // MR mode: gitlab configured
    if app.gitlab_client.is_some() {
        draw_mr_list(frame, app, inner, area);
        return;
    }

    // V1 mode: no gitlab config — original pane list
    if app.panes.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No opencode panes found.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Make sure opencode is running in a tmux pane.",
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
        lines.push(Line::from(vec![
            Span::styled("  ▪ ", Style::default().fg(Color::Cyan)),
            Span::styled(
                session_name.as_str(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        for &idx in pane_indices {
            let pane = &app.panes[idx];
            let is_selected = flat_idx == app.selected;
            flat_idx += 1;

            // Row 1: cursor + status badge + title
            let cursor = if is_selected { "▸ " } else { "  " };
            let badge = status_badge(&pane.status);

            let title_color = if is_selected {
                Color::White
            } else {
                Color::Gray
            };

            let mut spans = vec![
                Span::styled(
                    format!("    {}", cursor),
                    Style::default().fg(if is_selected {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    }),
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

            // Row 2: detail line — agent · model · time ago
            let mut detail_parts: Vec<Span> = Vec::new();

            // Agent
            detail_parts.push(Span::styled(
                pane.display_agent().to_string(),
                Style::default().fg(Color::DarkGray),
            ));

            // Separator
            detail_parts.push(Span::styled(
                " · ",
                Style::default().fg(Color::Indexed(238)),
            ));

            // Model
            detail_parts.push(Span::styled(
                pane.display_model().to_string(),
                Style::default().fg(Color::DarkGray),
            ));

            // Time ago (only for idle/unknown)
            if (pane.status == PaneStatus::Idle || pane.status == PaneStatus::Unknown)
                && let Some(ago) = pane.time_ago() {
                    detail_parts.push(Span::styled(
                        " · ",
                        Style::default().fg(Color::Indexed(238)),
                    ));
                    detail_parts.push(Span::styled(ago, Style::default().fg(Color::DarkGray)));
                }

            let mut detail_line = vec![Span::raw("          ")];
            detail_line.extend(detail_parts);
            lines.push(Line::from(detail_line));

            // Row 3: latest AI response preview
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
                        Span::styled("          ▹ ", Style::default().fg(Color::Green)),
                        Span::styled(truncated, Style::default().fg(Color::Indexed(250))),
                    ]));
                }
            }
        }

        // Spacing between groups
        lines.push(Line::from(""));
    }

    // Scroll to keep selected visible
    let visible_height = inner.height as usize;
    let scroll = compute_scroll(
        &lines,
        app.selected,
        &app.groups,
        &app.panes,
        visible_height,
    );

    let paragraph = Paragraph::new(lines).scroll((scroll as u16, 0));
    frame.render_widget(paragraph, inner);
}

fn draw_mr_list(frame: &mut Frame, app: &App, inner: Rect, area: Rect) {
    let mr_section_focused = matches!(app.selection_section, SelectionSection::MergeRequests);
    let mut lines: Vec<Line> = Vec::new();

    let mr_header_color = if mr_section_focused { Color::Cyan } else { Color::DarkGray };
    let mr_count = app.dashboard.linked_mrs.len();
    lines.push(Line::from(Span::styled(
        format!("  Merge Requests ({})", mr_count),
        Style::default().fg(mr_header_color).add_modifier(Modifier::BOLD),
    )));

    if app.dashboard.linked_mrs.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No open MRs. Press 'r' to refresh.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, linked) in app.dashboard.linked_mrs.iter().enumerate() {
            let is_selected = mr_section_focused && i == app.mr_selected;
            let cursor = if is_selected { "▸ " } else { "  " };
            let iid_str = format!("!{}", linked.mr.iid);
            let title_color = if is_selected { Color::White } else { Color::Gray };
            let iid_color = if is_selected { Color::Cyan } else { Color::DarkGray };
            let reserved = 2 + iid_str.len() + 1 + if linked.mr.draft { 9 } else { 0 } + 4;
            let max_title = (area.width as usize).saturating_sub(reserved + 4);
            let title = truncate(&linked.mr.title, max_title);
            let activity_dot = if linked.has_new_activity {
                Span::styled(" ● ", Style::default().fg(Color::Yellow))
            } else {
                Span::styled(" ○ ", Style::default().fg(Color::DarkGray))
            };
            let mut spans = vec![
                Span::styled(
                    format!("  {}", cursor),
                    Style::default().fg(if is_selected { Color::Cyan } else { Color::DarkGray }),
                ),
                Span::styled(iid_str, Style::default().fg(iid_color)),
                Span::raw(" "),
                Span::styled(title, Style::default().fg(title_color)),
            ];
            if linked.mr.draft {
                spans.push(Span::styled(
                    " [draft]",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM),
                ));
            }
            spans.push(activity_dot);
            if let Some(ref pane) = linked.tmux_pane {
                spans.push(compact_status_badge(&pane.status));
            }
            if is_selected {
                for s in &mut spans {
                    s.style = s.style.add_modifier(Modifier::BOLD);
                }
            }
            lines.push(Line::from(spans));
        }
    }

    if !app.dashboard.unlinked_instances.is_empty() {
        lines.push(Line::from(""));
        let unlinked_header_color = if !mr_section_focused { Color::Cyan } else { Color::DarkGray };
        let unlinked_count = app.dashboard.unlinked_instances.len();
        lines.push(Line::from(Span::styled(
            format!("  Opencode (no MR) ({})", unlinked_count),
            Style::default().fg(unlinked_header_color).add_modifier(Modifier::BOLD),
        )));
        for (i, unlinked) in app.dashboard.unlinked_instances.iter().enumerate() {
            let is_selected = !mr_section_focused && i == app.unlinked_selected;
            let cursor = if is_selected { "▸ " } else { "  " };
            let branch = unlinked.branch.as_deref().unwrap_or("unknown");
            let path_str = unlinked
                .worktree
                .as_ref()
                .map(|wt| format!(" ({})", shorten_path(&wt.path)))
                .unwrap_or_default();
            let title_color = if is_selected { Color::White } else { Color::Gray };
            let mut spans = vec![
                Span::styled(
                    format!("  {}", cursor),
                    Style::default().fg(if is_selected { Color::Cyan } else { Color::DarkGray }),
                ),
                Span::styled(branch.to_string(), Style::default().fg(title_color)),
                Span::styled(path_str, Style::default().fg(Color::DarkGray)),
                Span::styled(" ○ ", Style::default().fg(Color::DarkGray)),
                compact_status_badge(&unlinked.pane.status),
            ];
            if is_selected {
                for s in &mut spans {
                    s.style = s.style.add_modifier(Modifier::BOLD);
                }
            }
            lines.push(Line::from(spans));
        }
    }

    let visible_height = inner.height as usize;
    let scroll = if lines.len() > visible_height {
        let selected_line = if mr_section_focused {
            if app.dashboard.linked_mrs.is_empty() { 1 } else { 1 + app.mr_selected }
        } else {
            let mr_lines = if app.dashboard.linked_mrs.is_empty() { 2 } else { 1 + app.dashboard.linked_mrs.len() };
            mr_lines + 2 + app.unlinked_selected
        };
        if selected_line + 3 > visible_height {
            selected_line.saturating_sub(visible_height / 2)
        } else {
            0
        }
    } else {
        0
    };

    let paragraph = Paragraph::new(lines).scroll((scroll as u16, 0));
    frame.render_widget(paragraph, inner);
}

// ─── Detail panel ─────────────────────────────────────────────────────────────

fn draw_detail_panel(frame: &mut Frame, app: &App, area: Rect) {
    // MR mode: gitlab configured → show MR detail
    if app.gitlab_client.is_some() {
        draw_mr_detail_panel(frame, app, area);
        return;
    }

    // V1 mode: show opencode session detail
    let panel_title = if let Some(pane) = app.panes.get(app.selected) {
        format!(
            " {} — {}:{}.{} ",
            pane.display_title(),
            pane.session_name,
            pane.window_index,
            pane.pane_index,
        )
    } else {
        " detail ".to_string()
    };

    let block = Block::default()
        .title(Span::styled(
            &panel_title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::new(1, 1, 1, 0));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(detail) = &app.detail else {
        let msg = Paragraph::new(Span::styled(
            "  No session data available.",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(msg, inner);
        return;
    };

    // Split inner area: metadata header + message timeline
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(detail_header_height(detail)),
            Constraint::Min(3),
        ])
        .split(inner);

    draw_detail_header(frame, detail, chunks[0]);
    draw_message_timeline(frame, detail, chunks[1]);
}

// ─── MR Detail panel ─────────────────────────────────────────────────────────

fn draw_mr_detail_panel(frame: &mut Frame, app: &App, area: Rect) {
    let panel_title = if let Some(linked) = app.dashboard.linked_mrs.get(app.mr_selected) {
        let title = truncate(&linked.mr.title, area.width.saturating_sub(10) as usize);
        format!(" !{} {} ", linked.mr.iid, title)
    } else {
        " MR detail ".to_string()
    };

    let block = Block::default()
        .title(Span::styled(
            &panel_title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .padding(Padding::new(1, 1, 1, 0));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(linked) = app.dashboard.linked_mrs.get(app.mr_selected) else {
        let msg = Paragraph::new(Span::styled(
            "  No MR selected. Press 'r' to fetch MRs.",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(msg, inner);
        return;
    };

    let mr = &linked.mr;
    let mut lines: Vec<Line> = Vec::new();

    // ── Status section ──
    lines.push(Line::from(Span::styled(
        "  Status",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::DIM),
    )));

    let state_color = match mr.state.as_str() {
        "opened" => Color::Green,
        "merged" => Color::Yellow,
        "closed" => Color::Red,
        _ => Color::Gray,
    };
    lines.push(Line::from(vec![
        Span::styled("  state      ", Style::default().fg(Color::DarkGray)),
        Span::styled(&mr.state, Style::default().fg(state_color)),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  branch     ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} → {}", mr.source_branch, mr.target_branch),
            Style::default().fg(Color::Gray),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  author     ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("@{}", mr.author.username),
            Style::default().fg(Color::White),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  draft      ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if mr.draft { "yes" } else { "no" },
            Style::default().fg(Color::Gray),
        ),
    ]));

    if let Some(ref detail) = app.cached_mr_detail {
        if detail.iid == mr.iid {
            if let Some(ref merge_status) = detail.detailed_merge_status {
                lines.push(Line::from(vec![
                    Span::styled("  status     ", Style::default().fg(Color::DarkGray)),
                    Span::styled(merge_status.as_str(), Style::default().fg(Color::Gray)),
                ]));
            }
            if let Some(has_conflicts) = detail.has_conflicts {
                if has_conflicts {
                    lines.push(Line::from(vec![
                        Span::styled("  conflicts  ", Style::default().fg(Color::DarkGray)),
                        Span::styled("yes", Style::default().fg(Color::Red)),
                    ]));
                }
            }
            if let Some(ref pipeline) = detail.head_pipeline {
                let pipe_color = match pipeline.status.as_str() {
                    "success" => Color::Green,
                    "failed" => Color::Red,
                    "running" => Color::Cyan,
                    "pending" => Color::Yellow,
                    _ => Color::Gray,
                };
                lines.push(Line::from(vec![
                    Span::styled("  pipeline   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&pipeline.status, Style::default().fg(pipe_color)),
                ]));
            }
        }
    }

    // ── Links section ──
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Links",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::DIM),
    )));

    if let Some(ref wt) = linked.worktree {
        lines.push(Line::from(vec![
            Span::styled("  worktree   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                shorten_path(&wt.path),
                Style::default().fg(Color::White),
            ),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "  worktree   not found",
            Style::default().fg(Color::DarkGray),
        )));
    }

    if let Some(ref pane) = linked.tmux_pane {
        lines.push(Line::from(vec![
            Span::styled("  tmux       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}:{}.{}", pane.session_name, pane.window_index, pane.pane_index),
                Style::default().fg(Color::White),
            ),
            Span::raw("  "),
            compact_status_badge(&pane.status),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "  tmux       not running",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // ── Activity section ──
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Activity",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD | Modifier::DIM),
    )));

    let comments_str = if linked.has_new_activity {
        format!("{} (● new)", mr.user_notes_count)
    } else {
        mr.user_notes_count.to_string()
    };
    let comments_color = if linked.has_new_activity {
        Color::Yellow
    } else {
        Color::White
    };
    lines.push(Line::from(vec![
        Span::styled("  comments   ", Style::default().fg(Color::DarkGray)),
        Span::styled(comments_str, Style::default().fg(comments_color)),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  updated    ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format_date(&mr.updated_at),
            Style::default().fg(Color::White),
        ),
    ]));

    let max_url_len = area.width.saturating_sub(16) as usize;
    lines.push(Line::from(vec![
        Span::styled("  url        ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            truncate(&mr.web_url, max_url_len),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// How many lines the header needs.
fn detail_header_height(detail: &SessionDetail) -> u16 {
    let mut h: u16 = 5; // directory + tokens + messages + changes + blank separator
    if !detail.todos.is_empty() {
        h += 1 + detail.todos.len().min(6) as u16; // header + items (cap at 6)
    }
    h
}

fn draw_detail_header(frame: &mut Frame, detail: &SessionDetail, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    // Directory
    lines.push(Line::from(vec![
        Span::styled("  dir  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            shorten_path(&detail.directory),
            Style::default().fg(Color::White),
        ),
    ]));

    // Tokens
    lines.push(Line::from(vec![
        Span::styled("  tok  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format_tokens(detail.input_tokens),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(" in  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format_tokens(detail.output_tokens),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(" out", Style::default().fg(Color::DarkGray)),
    ]));

    // Messages
    lines.push(Line::from(vec![
        Span::styled("  msg  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            detail.message_count.to_string(),
            Style::default().fg(Color::White),
        ),
        Span::styled(" messages", Style::default().fg(Color::DarkGray)),
        if let Some(dur) = session_duration(detail) {
            Span::styled(
                format!("  ·  {}", dur),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::raw("")
        },
    ]));

    // File changes summary
    if detail.summary_files.unwrap_or(0) > 0 {
        lines.push(Line::from(vec![
            Span::styled("  Δ    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} files", detail.summary_files.unwrap_or(0)),
                Style::default().fg(Color::White),
            ),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("+{}", detail.summary_additions.unwrap_or(0)),
                Style::default().fg(Color::Green),
            ),
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("-{}", detail.summary_deletions.unwrap_or(0)),
                Style::default().fg(Color::Red),
            ),
        ]));
    }

    // Todos
    if !detail.todos.is_empty() {
        lines.push(Line::from(""));
        for todo in detail.todos.iter().take(6) {
            let (icon, color) = match todo.status.as_str() {
                "completed" => ("✓", Color::Green),
                "in_progress" => ("▸", Color::Cyan),
                "cancelled" => ("✗", Color::DarkGray),
                _ => ("○", Color::DarkGray),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", icon), Style::default().fg(color)),
                Span::styled(
                    truncate(&todo.content, area.width.saturating_sub(6) as usize),
                    Style::default().fg(if todo.status == "completed" {
                        Color::DarkGray
                    } else {
                        Color::Gray
                    }),
                ),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_message_timeline(frame: &mut Frame, detail: &SessionDetail, area: Rect) {
    if detail.messages.is_empty() {
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Section header
    lines.push(Line::from(Span::styled(
        "  ── messages ──",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    for msg in &detail.messages {
        let (role_label, role_color) = match msg.role.as_str() {
            "user" => ("▸ you", Color::Cyan),
            "assistant" => ("◂ ai ", Color::Green),
            _ => ("  ···", Color::DarkGray),
        };

        let time = format_timestamp(msg.timestamp);

        let mut spans = vec![
            Span::styled(format!("  {}", role_label), Style::default().fg(role_color)),
            Span::styled(format!("  {}", time), Style::default().fg(Color::DarkGray)),
        ];

        // Show token count for assistant messages
        if msg.role == "assistant" && msg.output_tokens > 0 {
            spans.push(Span::styled(
                format!("  {}tok", format_tokens(msg.output_tokens)),
                Style::default().fg(Color::Indexed(238)),
            ));
        }

        lines.push(Line::from(spans));

        // Text preview on next line (if available)
        if let Some(ref text) = msg.text_preview {
            let preview = text.lines().next().unwrap_or("");
            if !preview.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!(
                        "          {}",
                        truncate(preview, area.width.saturating_sub(12) as usize)
                    ),
                    Style::default().fg(Color::Indexed(245)),
                )));
            }
        }
    }

    // Scroll to show most recent messages (bottom)
    let visible = area.height as usize;
    let scroll = if lines.len() > visible {
        (lines.len() - visible) as u16
    } else {
        0
    };

    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(paragraph, area);
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn shorten_path(path: &str) -> String {
    if let Some(home) = dirs::home_dir()
        && let Some(rest) = path.strip_prefix(home.to_str().unwrap_or("")) {
            return format!("~{}", rest);
        }
    path.to_string()
}

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

fn format_timestamp(ts_ms: i64) -> String {
    // Convert epoch ms to HH:MM
    let secs = ts_ms / 1000;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    format!("{:02}:{:02}", hours, mins)
}

fn session_duration(detail: &SessionDetail) -> Option<String> {
    let created = detail.session_created?;
    let updated = detail.session_updated?;
    let elapsed_secs = (updated - created) / 1000;
    if elapsed_secs < 60 {
        Some(format!("{}s", elapsed_secs))
    } else if elapsed_secs < 3600 {
        Some(format!("{}m", elapsed_secs / 60))
    } else if elapsed_secs < 86400 {
        Some(format!(
            "{}h {}m",
            elapsed_secs / 3600,
            (elapsed_secs % 3600) / 60
        ))
    } else {
        Some(format!(
            "{}d {}h",
            elapsed_secs / 86400,
            (elapsed_secs % 86400) / 3600
        ))
    }
}

fn format_date(iso: &str) -> &str {
    if iso.len() >= 10 { &iso[..10] } else { iso }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max > 3 {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{}...", truncated)
    } else {
        s.chars().take(max).collect()
    }
}

// ─── Status badges ────────────────────────────────────────────────────────────

/// Returns a styled Span with a colored background badge for the status.
fn status_badge(status: &PaneStatus) -> Span<'static> {
    match status {
        PaneStatus::Busy => Span::styled(
            " ● BUSY ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Retry { .. } => Span::styled(
            " ⚠ RETRY ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Idle => Span::styled(
            " ⏸ IDLE ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Unknown => Span::styled(
            " ? no server ",
            Style::default().fg(Color::DarkGray).bg(Color::Indexed(236)),
        ),
    }
}

fn compact_status_badge(status: &PaneStatus) -> Span<'static> {
    match status {
        PaneStatus::Busy => Span::styled(" B ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
        PaneStatus::Retry { .. } => Span::styled(" R ", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)),
        PaneStatus::Idle => Span::styled(" I ", Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)),
        PaneStatus::Unknown => Span::styled("   ", Style::default()),
    }
}

// ─── Scroll ───────────────────────────────────────────────────────────────────

/// Compute scroll offset to keep the selected pane visible.
fn compute_scroll(
    lines: &[Line],
    selected: usize,
    groups: &[(String, Vec<usize>)],
    panes: &[crate::types::AgentPane],
    visible_height: usize,
) -> usize {
    // Each group: 1 header, then 2-3 lines per pane (3 if has last_response), then 1 blank
    let mut line_idx = 0;
    let mut flat = 0;
    for (_, pane_indices) in groups {
        line_idx += 1; // header
        for &idx in pane_indices {
            if flat == selected {
                if line_idx + 3 > visible_height {
                    return line_idx.saturating_sub(visible_height / 2);
                }
                return 0;
            }
            let pane_lines = if panes[idx].last_response.is_some() {
                3
            } else {
                2
            };
            line_idx += pane_lines;
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
