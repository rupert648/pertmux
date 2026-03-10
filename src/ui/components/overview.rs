use crate::client::ClientState;
use crate::types::PaneStatus;
use crate::ui::ACCENT;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
};

pub(crate) fn draw_overview_panel(frame: &mut Frame, state: &ClientState, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            " Projects ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let header = Row::new([
        Cell::from("  Name"),
        Cell::from("MRs"),
        Cell::from("Busy"),
        Cell::from("Idle"),
    ])
    .style(Style::default().fg(Color::DarkGray))
    .bottom_margin(0);

    let rows: Vec<Row> = state
        .snapshot
        .projects
        .iter()
        .enumerate()
        .map(|(i, proj)| {
            let is_active = i == state.active_project;
            let mr_count = proj.dashboard.linked_mrs.len();

            let project_paths: Vec<&str> = std::iter::once(proj.local_path.as_str())
                .chain(
                    proj.cached_worktrees
                        .iter()
                        .filter_map(|wt| wt.path.as_deref()),
                )
                .collect();
            let pane_belongs = |pane: &crate::types::AgentPane| {
                let p = pane.pane_path.trim_end_matches('/');
                project_paths.iter().any(|pp| p == pp.trim_end_matches('/'))
            };
            let busy_count = state
                .snapshot
                .panes
                .iter()
                .filter(|p| matches!(p.status, PaneStatus::Busy) && pane_belongs(p))
                .count();
            let idle_count = state
                .snapshot
                .panes
                .iter()
                .filter(|p| matches!(p.status, PaneStatus::Idle) && pane_belongs(p))
                .count();

            let marker = if is_active { "\u{25b8} " } else { "  " };
            let name_style = if is_active {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let mr_style = if mr_count > 0 {
                Style::default().fg(ACCENT)
            } else {
                Style::default().fg(Color::Indexed(238))
            };
            let busy_style = if busy_count > 0 {
                Style::default().fg(ACCENT)
            } else {
                Style::default().fg(Color::Indexed(238))
            };
            let idle_style = if idle_count > 0 {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Indexed(238))
            };

            Row::new([
                Cell::from(Span::styled(format!("{}{}", marker, proj.name), name_style)),
                Cell::from(Span::styled(mr_count.to_string(), mr_style)),
                Cell::from(Span::styled(busy_count.to_string(), busy_style)),
                Cell::from(Span::styled(idle_count.to_string(), idle_style)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Min(12),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(4),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1);

    frame.render_widget(table, area);
}
