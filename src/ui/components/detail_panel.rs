use super::pipeline::render_pipeline_dots;
use crate::app::SelectionSection;
use crate::client::ClientState;
use crate::protocol::ProjectSnapshot;
use crate::types::SessionDetail;
use crate::ui::helpers::{
    compact_status_badge, format_date, format_relative_time, format_timestamp, format_tokens,
    session_duration, shorten_path, truncate,
};
use crate::ui::{ProjectRenderData, ACCENT};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
    Frame,
};

pub(crate) fn draw_detail_panel_client(frame: &mut Frame, state: &ClientState, area: Rect) {
    if let Some(proj) = state.snapshot.projects.get(state.active_project) {
        let section = state
            .selection_section
            .get(state.active_project)
            .unwrap_or(&SelectionSection::MergeRequests);
        draw_mr_detail_panel_client(
            frame,
            proj,
            &state.snapshot.panes,
            *state.mr_selected.get(state.active_project).unwrap_or(&0),
            *state
                .worktree_selected
                .get(state.active_project)
                .unwrap_or(&0),
            section,
            area,
        );
        return;
    }

    let panel_title = if let Some(pane) = state.snapshot.panes.get(state.selected) {
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

    let Some(detail) = &state.snapshot.detail else {
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

fn draw_mr_detail_panel_client(
    frame: &mut Frame,
    proj: &ProjectSnapshot,
    panes: &[crate::types::AgentPane],
    mr_selected: usize,
    worktree_selected: usize,
    section: &SelectionSection,
    area: Rect,
) {
    let render =
        ProjectRenderData::from_snapshot(proj, panes, mr_selected, worktree_selected, section);
    draw_mr_detail_panel_render(frame, &render, area);
}

fn draw_mr_detail_panel_render(frame: &mut Frame, proj: &ProjectRenderData<'_>, area: Rect) {
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

    if let Some(detail) = proj.cached_mr_detail {
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
                lines.extend(render_pipeline_dots(proj.cached_pipeline_jobs));
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
            Span::styled(shorten_path(&wt.path), Style::default().fg(Color::White)),
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

    if !proj.cached_threads.is_empty() && proj.cached_threads_iid == Some(mr.iid) {
        lines.push(Line::from(""));

        let total = proj.cached_threads.len();
        let resolvable_count = proj.cached_threads.iter().filter(|t| t.resolvable).count();
        let resolved_count = proj
            .cached_threads
            .iter()
            .filter(|t| t.resolvable && t.resolved)
            .count();

        let header_text = if resolvable_count > 0 {
            format!("  Discussions ({}/{})", resolved_count, resolvable_count)
        } else {
            format!("  Discussions ({})", total)
        };
        lines.push(Line::from(Span::styled(
            header_text,
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::DIM),
        )));

        let max_body_len = area.width.saturating_sub(8) as usize;

        for thread in proj.cached_threads {
            let first = match thread.notes.first() {
                Some(n) => n,
                None => continue,
            };

            let indicator = if !thread.resolvable {
                Span::styled("  \u{00b7} ", Style::default().fg(Color::DarkGray))
            } else if thread.resolved {
                Span::styled("  \u{2713} ", Style::default().fg(Color::Green))
            } else {
                Span::styled("  \u{2717} ", Style::default().fg(Color::Red))
            };

            let mut header_spans = vec![
                indicator,
                Span::styled(
                    format!("@{}", first.author.username),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(" \u{00b7} {}", format_relative_time(&first.created_at)),
                    Style::default().fg(Color::DarkGray),
                ),
            ];

            if let Some(ref path) = thread.file_path {
                let file_display = if let Some(name) = path.rsplit('/').next() {
                    match thread.line {
                        Some(ln) => format!("{name}:{ln}"),
                        None => name.to_string(),
                    }
                } else {
                    path.clone()
                };
                header_spans.push(Span::styled(
                    format!(" \u{00b7} {file_display}"),
                    Style::default().fg(Color::Indexed(245)),
                ));
            }

            lines.push(Line::from(header_spans));

            let body_preview = first.body.lines().next().unwrap_or("");
            if !body_preview.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("    {}", truncate(body_preview, max_body_len)),
                    Style::default().fg(Color::Indexed(245)),
                )));
            }

            for reply in thread.notes.iter().skip(1) {
                if reply.system {
                    continue;
                }
                lines.push(Line::from(vec![
                    Span::styled("    \u{2514} ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("@{}", reply.author.username),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        format!(" \u{00b7} {}", format_relative_time(&reply.created_at)),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                let reply_preview = reply.body.lines().next().unwrap_or("");
                if !reply_preview.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("      {}", truncate(reply_preview, max_body_len)),
                        Style::default().fg(Color::Indexed(245)),
                    )));
                }
            }
        }
    }

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
