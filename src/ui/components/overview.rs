use crate::client::ClientState;
use crate::project_sort::{ORDER, ProjectSort};
use crate::project_stats::{ProjectStats, build_sorted_project_stats};
use crate::ui::{ACCENT, CURSOR_BG, DIM};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
};

/// Rows of fixed chrome above/below the scrollable body:
/// top border (1) + bottom border (1) + header row (1) = 3.
const CHROME_ROWS: u16 = 3;

pub(crate) fn draw_overview_panel(frame: &mut Frame, state: &ClientState, area: Rect) {
    let sort_col = state.project_sort_col;
    let sort_desc = state.project_sort_desc;
    let cursor_col = state.project_cursor_col;
    let focused = state.project_focused;

    let arrow = if sort_desc { "↓" } else { "↑" };
    let title = format!(" Projects [{} {}] ", arrow, sort_col.label());

    // Publish viewport height so the key handler can clamp scrolling.
    let visible_rows = area.height.saturating_sub(CHROME_ROWS);
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

    let header = Row::new(
        ORDER
            .iter()
            .map(|col| header_cell(*col, sort_col, cursor_col, focused))
            .collect::<Vec<_>>(),
    )
    .bottom_margin(0);

    // Build + sort once, shared with the client.
    let stats = build_sorted_project_stats(&state.snapshot, sort_col, sort_desc);

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
            data_row(s, is_active, is_cursor_row)
        })
        .collect();

    let widths = [
        Constraint::Min(10),   // Name
        Constraint::Length(4), // MRs
        Constraint::Length(3), // WT
        Constraint::Length(3), // OC
        Constraint::Length(4), // Busy
        Constraint::Length(4), // Idle
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1);

    frame.render_widget(table, area);
}

fn header_label(col: ProjectSort) -> &'static str {
    match col {
        ProjectSort::Name => "  Name",
        ProjectSort::Mrs => "MRs",
        ProjectSort::Wt => "WT",
        ProjectSort::Oc => "OC",
        ProjectSort::Busy => "Busy",
        ProjectSort::Idle => "Idle",
    }
}

fn header_cell(
    col: ProjectSort,
    sort_col: ProjectSort,
    cursor_col: ProjectSort,
    focused: bool,
) -> Cell<'static> {
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
    Cell::from(Span::styled(header_label(col).to_string(), style))
}

/// Style a count cell: `on` colour when non-zero, dim otherwise.
fn count_style(n: usize, on: Color) -> Style {
    if n > 0 {
        Style::default().fg(on)
    } else {
        Style::default().fg(DIM)
    }
}

fn data_row(s: &ProjectStats, is_active: bool, is_cursor_row: bool) -> Row<'static> {
    let marker = if is_active { "\u{25b8} " } else { "  " };
    let name_style = if is_active {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let oc = s.oc();

    let row = Row::new([
        Cell::from(Span::styled(format!("{}{}", marker, s.name), name_style)),
        Cell::from(Span::styled(s.mrs.to_string(), count_style(s.mrs, ACCENT))),
        Cell::from(Span::styled(s.wt.to_string(), count_style(s.wt, ACCENT))),
        Cell::from(Span::styled(oc.to_string(), count_style(oc, ACCENT))),
        Cell::from(Span::styled(
            s.busy.to_string(),
            count_style(s.busy, ACCENT),
        )),
        Cell::from(Span::styled(
            s.idle.to_string(),
            count_style(s.idle, Color::Green),
        )),
    ]);
    if is_cursor_row {
        row.style(Style::default().bg(CURSOR_BG))
    } else {
        row
    }
}
