use crate::client::{ClientState, ProjectSort};
use crate::types::PaneStatus;
use crate::ui::ACCENT;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
};

struct ProjectStats {
    canonical_idx: usize,
    name: String,
    mrs: usize,
    busy: usize,
    idle: usize,
}

impl ProjectStats {
    fn oc(&self) -> usize {
        self.busy + self.idle
    }
}

pub(crate) fn draw_overview_panel(frame: &mut Frame, state: &ClientState, area: Rect) {
    let sort_col = state.project_sort_col;
    let sort_desc = state.project_sort_desc;
    let cursor_col = state.project_cursor_col;
    let focused = state.project_focused;

    let arrow = if sort_desc { "↓" } else { "↑" };
    let title = format!(" Projects [{} {}] ", arrow, sort_col.label());

    // Persist viewport height for the key handler to clamp scrolling.
    // 2 = top + bottom border, 1 = header row.
    let visible_rows = area.height.saturating_sub(3);
    state.overview_height.set(visible_rows);

    let border_style = if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let header_cell = |label: &'static str, col: ProjectSort| {
        let is_sort = col == sort_col;
        let is_cursor = focused && col == cursor_col;
        let mut style = if is_sort {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        if is_cursor {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        Cell::from(Span::styled(label.to_string(), style))
    };

    let header = Row::new([
        header_cell("  Name", ProjectSort::Name),
        header_cell("MRs", ProjectSort::Mrs),
        header_cell("OC", ProjectSort::Oc),
        header_cell("Busy", ProjectSort::Busy),
        header_cell("Idle", ProjectSort::Idle),
    ])
    .bottom_margin(0);

    // Build stats once.
    let mut stats: Vec<ProjectStats> = state
        .snapshot
        .projects
        .iter()
        .enumerate()
        .map(|(i, proj)| {
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
            let busy = state
                .snapshot
                .panes
                .iter()
                .filter(|p| matches!(p.status, PaneStatus::Busy) && pane_belongs(p))
                .count();
            let idle = state
                .snapshot
                .panes
                .iter()
                .filter(|p| matches!(p.status, PaneStatus::Idle) && pane_belongs(p))
                .count();
            ProjectStats {
                canonical_idx: i,
                name: proj.name.clone(),
                mrs: proj.dashboard.linked_mrs.len(),
                busy,
                idle,
            }
        })
        .collect();

    // Sort respecting persisted direction; ties broken by name asc.
    stats.sort_by(|a, b| {
        let cmp = match sort_col {
            ProjectSort::Name => a.name.cmp(&b.name),
            ProjectSort::Mrs => a.mrs.cmp(&b.mrs),
            ProjectSort::Oc => a.oc().cmp(&b.oc()),
            ProjectSort::Busy => a.busy.cmp(&b.busy),
            ProjectSort::Idle => a.idle.cmp(&b.idle),
        };
        let cmp = if sort_desc { cmp.reverse() } else { cmp };
        cmp.then_with(|| a.name.cmp(&b.name))
    });

    // Slice to viewport.
    let total = stats.len();
    let scroll = state.project_scroll.min(total);
    let end = (scroll + visible_rows as usize).min(total);

    let rows: Vec<Row> = stats[scroll..end]
        .iter()
        .enumerate()
        .map(|(offset, s)| {
            let display_idx = scroll + offset;
            let is_active = s.canonical_idx == state.active_project;
            let is_cursor_row = focused && display_idx == state.project_cursor_row;
            let oc = s.oc();

            let marker = if is_active { "\u{25b8} " } else { "  " };
            let name_style = if is_active {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let dim = Style::default().fg(Color::Indexed(242));
            let mr_style = if s.mrs > 0 {
                Style::default().fg(ACCENT)
            } else {
                dim
            };
            let oc_style = if oc > 0 {
                Style::default().fg(ACCENT)
            } else {
                dim
            };
            let busy_style = if s.busy > 0 {
                Style::default().fg(ACCENT)
            } else {
                dim
            };
            let idle_style = if s.idle > 0 {
                Style::default().fg(Color::Green)
            } else {
                dim
            };

            let row = Row::new([
                Cell::from(Span::styled(format!("{}{}", marker, s.name), name_style)),
                Cell::from(Span::styled(s.mrs.to_string(), mr_style)),
                Cell::from(Span::styled(oc.to_string(), oc_style)),
                Cell::from(Span::styled(s.busy.to_string(), busy_style)),
                Cell::from(Span::styled(s.idle.to_string(), idle_style)),
            ]);
            if is_cursor_row {
                row.style(Style::default().bg(Color::Indexed(237)))
            } else {
                row
            }
        })
        .collect();

    let widths = [
        Constraint::Min(10),
        Constraint::Length(4),
        Constraint::Length(3),
        Constraint::Length(4),
        Constraint::Length(4),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1);

    frame.render_widget(table, area);
}
