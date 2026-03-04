use crate::app::{App, ProjectState, SelectionSection};
use crate::gitlab::types::PipelineJob;
use crate::types::{PaneStatus, SessionDetail};
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Tabs,
    },
    Frame,
};

const ACCENT: Color = Color::Rgb(255, 140, 0);
const NOTIFICATION_DURATION: std::time::Duration = std::time::Duration::from_secs(2);

fn is_landscape(area: Rect) -> bool {
    area.width >= area.height * 2
}

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    if is_landscape(area) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);
        draw_list_panel(frame, app, chunks[0]);
        draw_detail_panel(frame, app, chunks[1]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);
        draw_list_panel(frame, app, chunks[0]);
        draw_detail_panel(frame, app, chunks[1]);
    }

    draw_notification(frame, app, area);
}

// ─── List panel ───────────────────────────────────────────────────────────────

fn draw_list_panel(frame: &mut Frame, app: &App, area: Rect) {
    let title_right = if let Some(proj) = app.active_project() {
        let mr_count = proj.dashboard.linked_mrs.len();
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

    let hint_bottom = if app.has_projects() {
        let mut hints = vec![
            Span::styled(" \u{2191}\u{2193}", Style::default().fg(ACCENT)),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("jk", Style::default().fg(ACCENT)),
            Span::styled(" nav  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", Style::default().fg(ACCENT)),
            Span::styled(" switch  ", Style::default().fg(Color::DarkGray)),
            Span::styled("\u{23ce}", Style::default().fg(ACCENT)),
            Span::styled(" focus  ", Style::default().fg(Color::DarkGray)),
            Span::styled("r", Style::default().fg(ACCENT)),
            Span::styled(" refresh  ", Style::default().fg(Color::DarkGray)),
            Span::styled("o", Style::default().fg(ACCENT)),
            Span::styled(" open  ", Style::default().fg(Color::DarkGray)),
            Span::styled("b", Style::default().fg(ACCENT)),
            Span::styled(" branch  ", Style::default().fg(Color::DarkGray)),
        ];
        if app.projects.len() > 1 {
            hints.push(Span::styled("h", Style::default().fg(ACCENT)));
            hints.push(Span::styled("/", Style::default().fg(Color::DarkGray)));
            hints.push(Span::styled("l", Style::default().fg(ACCENT)));
            hints.push(Span::styled(" tab  ", Style::default().fg(Color::DarkGray)));
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
            Span::styled("r", Style::default().fg(ACCENT)),
            Span::styled(" refresh  ", Style::default().fg(Color::DarkGray)),
            Span::styled("q", Style::default().fg(ACCENT)),
            Span::styled(" quit ", Style::default().fg(Color::DarkGray)),
        ])
    };

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " pert",
                Style::default()
                    .fg(ACCENT)
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

    if let Some(proj) = app.active_project() {
        if app.projects.len() > 1 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(inner);
            draw_project_tabs(frame, app, chunks[0]);
            draw_mr_sections(frame, proj, chunks[1]);
        } else {
            draw_mr_sections(frame, proj, inner);
        }
        return;
    }

    // V1 mode: no projects — original pane list
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
        lines.push(Line::from(vec![
            Span::styled("  \u{25aa} ", Style::default().fg(ACCENT)),
            Span::styled(
                session_name.as_str(),
                Style::default()
                    .fg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        for &idx in pane_indices {
            let pane = &app.panes[idx];
            let is_selected = flat_idx == app.selected;
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
                    Style::default().fg(if is_selected {
                        ACCENT
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
        app.selected,
        &app.groups,
        &app.panes,
        visible_height,
    );

    let paragraph = Paragraph::new(lines).scroll((scroll as u16, 0));
    frame.render_widget(paragraph, inner);
}

fn draw_project_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = app
        .projects
        .iter()
        .map(|p| Line::from(format!(" {} ", p.config.name)))
        .collect();

    let tabs = Tabs::new(titles)
        .highlight_style(
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .select(app.active_project)
        .style(Style::default().fg(Color::DarkGray))
        .divider(Span::styled("\u{2502}", Style::default().fg(Color::Indexed(238))));

    frame.render_widget(tabs, area);
}

fn draw_mr_sections(frame: &mut Frame, proj: &ProjectState, area: Rect) {
    let mr_focused = matches!(proj.selection_section, SelectionSection::MergeRequests);
    let has_unlinked = !proj.dashboard.unlinked_instances.is_empty();

    let chunks = if has_unlinked {
        let mr_count = proj.dashboard.linked_mrs.len().max(1) as u16;
        let ul_count = proj.dashboard.unlinked_instances.len().max(1) as u16;
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Ratio(mr_count as u32, mr_count as u32 + ul_count as u32),
                Constraint::Ratio(ul_count as u32, mr_count as u32 + ul_count as u32),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100), Constraint::Min(0)])
            .split(area)
    };

    draw_mr_block(frame, proj, chunks[0], mr_focused);

    if has_unlinked {
        draw_unlinked_block(frame, proj, chunks[1], !mr_focused);
    }
}

fn draw_mr_block(frame: &mut Frame, proj: &ProjectState, area: Rect, focused: bool) {
    let border_color = if focused { ACCENT } else { Color::Indexed(238) };
    let mr_count = proj.dashboard.linked_mrs.len();

    let block = Block::default()
        .title(Line::from(vec![Span::styled(
            format!(" Merge Requests ({}) ", mr_count),
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        )]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let section_inner = block.inner(area);
    frame.render_widget(block, area);

    if section_inner.height == 0 || section_inner.width == 0 {
        return;
    }

    if mr_count == 0 {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No open MRs. Press 'r' to refresh.",
                Style::default().fg(Color::DarkGray),
            ))),
            section_inner,
        );
        return;
    }

    let card_h: u16 = 4;
    let total_content = mr_count as u16 * card_h;
    let selected_y = proj.mr_selected as u16 * card_h;

    let scroll: u16 = if total_content <= section_inner.height {
        0
    } else {
        let max_scroll = total_content.saturating_sub(section_inner.height);
        let ideal = selected_y.saturating_sub(section_inner.height / 2);
        ideal.min(max_scroll)
    };

    for (i, linked) in proj.dashboard.linked_mrs.iter().enumerate() {
        let card_y = i as u16 * card_h;
        let sy = card_y as i32 - scroll as i32;
        if sy + card_h as i32 <= 0 || sy >= section_inner.height as i32 {
            continue;
        }
        if sy < 0 || sy as u16 + card_h > section_inner.height {
            continue;
        }
        let ay = section_inner.y + sy as u16;
        let is_selected = focused && i == proj.mr_selected;
        let rect = Rect::new(section_inner.x, ay, section_inner.width, card_h);
        render_mr_card(frame, linked, rect, is_selected);
    }

    if total_content > section_inner.height {
        let mut scrollbar_state = ScrollbarState::new(mr_count).position(proj.mr_selected);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

fn draw_unlinked_block(frame: &mut Frame, proj: &ProjectState, area: Rect, focused: bool) {
    let border_color = if focused { ACCENT } else { Color::Indexed(238) };
    let ul_count = proj.dashboard.unlinked_instances.len();

    let block = Block::default()
        .title(Line::from(vec![Span::styled(
            format!(" Opencode ({}) ", ul_count),
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        )]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let section_inner = block.inner(area);
    frame.render_widget(block, area);

    if section_inner.height == 0 || section_inner.width == 0 || ul_count == 0 {
        return;
    }

    let card_h: u16 = 4;
    let total_content = ul_count as u16 * card_h;
    let selected_y = proj.unlinked_selected as u16 * card_h;

    let scroll: u16 = if total_content <= section_inner.height {
        0
    } else {
        let max_scroll = total_content.saturating_sub(section_inner.height);
        let ideal = selected_y.saturating_sub(section_inner.height / 2);
        ideal.min(max_scroll)
    };

    for (i, unlinked) in proj.dashboard.unlinked_instances.iter().enumerate() {
        let card_y = i as u16 * card_h;
        let sy = card_y as i32 - scroll as i32;
        if sy + card_h as i32 <= 0 || sy >= section_inner.height as i32 {
            continue;
        }
        if sy < 0 || sy as u16 + card_h > section_inner.height {
            continue;
        }
        let ay = section_inner.y + sy as u16;
        let is_selected = focused && i == proj.unlinked_selected;
        let rect = Rect::new(section_inner.x, ay, section_inner.width, card_h);
        render_unlinked_card(frame, unlinked, rect, is_selected);
    }

    if total_content > section_inner.height {
        let mut scrollbar_state = ScrollbarState::new(ul_count).position(proj.unlinked_selected);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

fn render_mr_card(
    frame: &mut Frame,
    linked: &crate::linking::LinkedMergeRequest,
    rect: Rect,
    is_selected: bool,
) {
    let border_color = if is_selected {
        ACCENT
    } else {
        Color::Indexed(238)
    };

    let iid_label = format!(" !{} ", linked.mr.iid);
    let iid_style = if is_selected {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(Line::from(Span::styled(iid_label, iid_style)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let card_inner = block.inner(rect);
    frame.render_widget(block, rect);

    if card_inner.width == 0 || card_inner.height == 0 {
        return;
    }

    let title_color = if is_selected {
        Color::White
    } else {
        Color::Gray
    };
    let content_w = card_inner.width as usize;
    let draft_space = if linked.mr.draft { 9 } else { 0 };
    let title = truncate(&linked.mr.title, content_w.saturating_sub(draft_space));

    let mut title_spans = vec![Span::styled(title, Style::default().fg(title_color))];
    if linked.mr.draft {
        title_spans.push(Span::styled(
            " [draft]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::DIM),
        ));
    }
    if is_selected {
        for s in &mut title_spans {
            s.style = s.style.add_modifier(Modifier::BOLD);
        }
    }

    let (icon, text, color) = merge_status_display(
        linked.mr.detailed_merge_status.as_deref(),
        linked.mr.has_conflicts,
    );
    let mut status_spans: Vec<Span> = vec![
        Span::styled(format!("{} {}", icon, text), Style::default().fg(color)),
        Span::styled(" \u{00b7} ", Style::default().fg(Color::Indexed(238))),
        Span::styled(
            format!("{} comments", linked.mr.user_notes_count),
            Style::default().fg(Color::DarkGray),
        ),
    ];
    if linked.has_new_activity {
        status_spans.push(Span::styled(
            " \u{00b7} ",
            Style::default().fg(Color::Indexed(238)),
        ));
        status_spans.push(Span::styled(
            "\u{25cf} new",
            Style::default().fg(Color::Yellow),
        ));
    }
    if let Some(ref pane) = linked.tmux_pane {
        status_spans.push(Span::raw(" "));
        status_spans.push(compact_status_badge(&pane.status));
    }

    let content = vec![Line::from(title_spans), Line::from(status_spans)];
    frame.render_widget(Paragraph::new(content), card_inner);
}

fn render_unlinked_card(
    frame: &mut Frame,
    unlinked: &crate::linking::UnlinkedInstance,
    rect: Rect,
    is_selected: bool,
) {
    let border_color = if is_selected {
        ACCENT
    } else {
        Color::Indexed(238)
    };

    let branch = unlinked.branch.as_deref().unwrap_or("unknown");
    let label_style = if is_selected {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .title(Line::from(Span::styled(
            format!(" {} ", branch),
            label_style,
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let card_inner = block.inner(rect);
    frame.render_widget(block, rect);

    if card_inner.width == 0 || card_inner.height == 0 {
        return;
    }

    let mut info_spans: Vec<Span> = Vec::new();
    if let Some(ref wt) = unlinked.worktree {
        info_spans.push(Span::styled(
            shorten_path(&wt.path),
            Style::default().fg(Color::DarkGray),
        ));
        info_spans.push(Span::styled(
            " \u{00b7} ",
            Style::default().fg(Color::Indexed(238)),
        ));
    }
    info_spans.push(compact_status_badge(&unlinked.pane.status));

    let content = vec![Line::from(vec![]), Line::from(info_spans)];
    frame.render_widget(Paragraph::new(content), card_inner);
}

fn merge_status_display(
    status: Option<&str>,
    has_conflicts: Option<bool>,
) -> (&'static str, &'static str, Color) {
    if has_conflicts == Some(true) {
        return ("\u{2717}", "conflicts", Color::Red);
    }
    match status {
        Some("mergeable") => ("\u{2713}", "mergeable", Color::Green),
        Some("not_approved") => ("\u{25cb}", "not approved", Color::Yellow),
        Some("checking") => ("\u{29d7}", "checking", ACCENT),
        Some("ci_must_pass") | Some("ci_still_running") => ("\u{29d7}", "CI running", ACCENT),
        Some("broken_status") => ("\u{2717}", "broken", Color::Red),
        Some("need_rebase") => ("\u{21bb}", "needs rebase", Color::Yellow),
        Some("blocked_status") => ("\u{2298}", "blocked", Color::Red),
        Some("discussions_not_resolved") => ("\u{25ce}", "discussions open", Color::Yellow),
        Some("draft_status") => ("\u{25c7}", "draft", Color::DarkGray),
        Some("not_open") => ("\u{2500}", "closed", Color::DarkGray),
        Some(other) => ("?", leak_status(other), Color::DarkGray),
        None => ("\u{2500}", "unknown", Color::DarkGray),
    }
}

fn leak_status(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn render_pipeline_dots(jobs: &[PipelineJob]) -> Vec<Line<'static>> {
    let mut stages: Vec<(String, Vec<&PipelineJob>)> = Vec::new();
    for job in jobs {
        if let Some(existing) = stages.iter_mut().find(|(s, _)| s == &job.stage) {
            existing.1.push(job);
        } else {
            stages.push((job.stage.clone(), vec![job]));
        }
    }

    let mut spans: Vec<Span> = vec![Span::styled(
        "  jobs       ",
        Style::default().fg(Color::DarkGray),
    )];
    for (i, (_stage, stage_jobs)) in stages.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default()));
        }
        for job in stage_jobs {
            spans.push(Span::styled(
                "\u{25cf}",
                Style::default().fg(job_status_color(&job.status)),
            ));
        }
    }

    let mut lines = vec![Line::from(spans)];

    let failed: Vec<&PipelineJob> = jobs
        .iter()
        .filter(|j| j.status == "failed" && !j.allow_failure)
        .collect();
    if !failed.is_empty() {
        let names: String = failed
            .iter()
            .map(|j| j.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(Line::from(vec![
            Span::styled(
                "             \u{2717} ",
                Style::default().fg(Color::Red),
            ),
            Span::styled(names, Style::default().fg(Color::Red)),
        ]));
    }

    lines
}

fn job_status_color(status: &str) -> Color {
    match status {
        "success" => Color::Green,
        "failed" => Color::Red,
        "running" => ACCENT,
        "pending" | "preparing" | "waiting_for_resource" => Color::Yellow,
        "manual" => Color::Rgb(128, 90, 213),
        "canceled" | "canceling" | "skipped" => Color::DarkGray,
        "created" => Color::Gray,
        _ => Color::Gray,
    }
}

// ─── Detail panel ─────────────────────────────────────────────────────────────

fn draw_detail_panel(frame: &mut Frame, app: &App, area: Rect) {
    if let Some(proj) = app.active_project() {
        draw_mr_detail_panel(frame, proj, area);
        return;
    }

    // V1 mode: show opencode session detail
    let panel_title = if let Some(pane) = app.panes.get(app.selected) {
        format!(
            " {} \u{2014} {}:{}.{} ",
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

fn draw_mr_detail_panel(frame: &mut Frame, proj: &ProjectState, area: Rect) {
    let panel_title = if let Some(linked) = proj.dashboard.linked_mrs.get(proj.mr_selected) {
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

    let Some(linked) = proj.dashboard.linked_mrs.get(proj.mr_selected) else {
        let msg = Paragraph::new(Span::styled(
            "  No MR selected. Press 'r' to fetch MRs.",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(msg, inner);
        return;
    };

    let mr = &linked.mr;
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        "  Status",
        Style::default()
            .fg(ACCENT)
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
            format!("{} \u{2192} {}", mr.source_branch, mr.target_branch),
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

    if let Some(ref detail) = proj.cached_mr_detail {
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
                    "running" => ACCENT,
                    "pending" => Color::Yellow,
                    _ => Color::Gray,
                };
                lines.push(Line::from(vec![
                    Span::styled("  pipeline   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&pipeline.status, Style::default().fg(pipe_color)),
                ]));
            }

            if !proj.cached_pipeline_jobs.is_empty() {
                lines.extend(render_pipeline_dots(&proj.cached_pipeline_jobs));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Links",
        Style::default()
            .fg(ACCENT)
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
                format!(
                    "{}:{}.{}",
                    pane.session_name, pane.window_index, pane.pane_index
                ),
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

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Activity",
        Style::default()
            .fg(ACCENT)
            .add_modifier(Modifier::BOLD | Modifier::DIM),
    )));

    let comments_str = if linked.has_new_activity {
        format!("{} (\u{25cf} new)", mr.user_notes_count)
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

fn detail_header_height(detail: &SessionDetail) -> u16 {
    let mut h: u16 = 5;
    if !detail.todos.is_empty() {
        h += 1 + detail.todos.len().min(6) as u16;
    }
    h
}

fn draw_detail_header(frame: &mut Frame, detail: &SessionDetail, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("  dir  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            shorten_path(&detail.directory),
            Style::default().fg(Color::White),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  tok  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format_tokens(detail.input_tokens),
            Style::default().fg(ACCENT),
        ),
        Span::styled(" in  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format_tokens(detail.output_tokens),
            Style::default().fg(ACCENT),
        ),
        Span::styled(" out", Style::default().fg(Color::DarkGray)),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  msg  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            detail.message_count.to_string(),
            Style::default().fg(Color::White),
        ),
        Span::styled(" messages", Style::default().fg(Color::DarkGray)),
        if let Some(dur) = session_duration(detail) {
            Span::styled(
                format!("  \u{00b7}  {}", dur),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::raw("")
        },
    ]));

    if detail.summary_files.unwrap_or(0) > 0 {
        lines.push(Line::from(vec![
            Span::styled("  \u{0394}    ", Style::default().fg(Color::DarkGray)),
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

    if !detail.todos.is_empty() {
        lines.push(Line::from(""));
        for todo in detail.todos.iter().take(6) {
            let (icon, color) = match todo.status.as_str() {
                "completed" => ("\u{2713}", Color::Green),
                "in_progress" => ("\u{25b8}", ACCENT),
                "cancelled" => ("\u{2717}", Color::DarkGray),
                _ => ("\u{25cb}", Color::DarkGray),
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

    lines.push(Line::from(Span::styled(
        "  \u{2500}\u{2500} messages \u{2500}\u{2500}",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    for msg in &detail.messages {
        let (role_label, role_color) = match msg.role.as_str() {
            "user" => ("\u{25b8} you", ACCENT),
            "assistant" => ("\u{25c2} ai ", Color::Green),
            _ => ("  \u{00b7}\u{00b7}\u{00b7}", Color::DarkGray),
        };

        let time = format_timestamp(msg.timestamp);

        let mut spans = vec![
            Span::styled(format!("  {}", role_label), Style::default().fg(role_color)),
            Span::styled(format!("  {}", time), Style::default().fg(Color::DarkGray)),
        ];

        if msg.role == "assistant" && msg.output_tokens > 0 {
            spans.push(Span::styled(
                format!("  {}tok", format_tokens(msg.output_tokens)),
                Style::default().fg(Color::Indexed(238)),
            ));
        }

        lines.push(Line::from(spans));

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

    let visible = area.height as usize;
    let scroll = if lines.len() > visible {
        (lines.len() - visible) as u16
    } else {
        0
    };

    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(paragraph, area);
}

// ─── Notification toast ───────────────────────────────────────────────────────

fn draw_notification(frame: &mut Frame, app: &App, area: Rect) {
    let Some((ref msg, at)) = app.notification else {
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

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn shorten_path(path: &str) -> String {
    if let Some(home) = dirs::home_dir()
        && let Some(rest) = path.strip_prefix(home.to_str().unwrap_or(""))
    {
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
    if iso.len() >= 10 {
        &iso[..10]
    } else {
        iso
    }
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

fn status_badge(status: &PaneStatus) -> Span<'static> {
    match status {
        PaneStatus::Busy => Span::styled(
            " \u{25cf} BUSY ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Retry { .. } => Span::styled(
            " \u{26a0} RETRY ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Idle => Span::styled(
            " \u{23f8} IDLE ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Unknown => Span::styled(
            " ? no server ",
            Style::default()
                .fg(Color::DarkGray)
                .bg(Color::Indexed(236)),
        ),
    }
}

fn compact_status_badge(status: &PaneStatus) -> Span<'static> {
    match status {
        PaneStatus::Busy => Span::styled(
            " B ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Retry { .. } => Span::styled(
            " R ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Idle => Span::styled(
            " I ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        PaneStatus::Unknown => Span::styled("   ", Style::default()),
    }
}

// ─── Scroll ───────────────────────────────────────────────────────────────────

fn compute_scroll(
    lines: &[Line],
    selected: usize,
    groups: &[(String, Vec<usize>)],
    panes: &[crate::types::AgentPane],
    visible_height: usize,
) -> usize {
    let mut line_idx = 0;
    let mut flat = 0;
    for (_, pane_indices) in groups {
        line_idx += 1;
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
        line_idx += 1;
    }
    if lines.len() > visible_height {
        lines.len() - visible_height
    } else {
        0
    }
}
