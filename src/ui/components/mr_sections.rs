use std::collections::HashMap;
use std::path::PathBuf;

use super::cards::{render_mr_card, render_worktree_card};
use crate::app::SelectionSection;
use crate::protocol::ProjectSnapshot;
use crate::types::AgentPane;
use crate::ui::{ProjectRenderData, ACCENT};
use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};

pub(crate) fn draw_mr_sections_client(
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
    draw_mr_sections_render(frame, &render, area);
}

fn draw_mr_sections_render(frame: &mut Frame, proj: &ProjectRenderData<'_>, area: Rect) {
    let mr_count = proj.dashboard.linked_mrs.len().max(1) as u16;
    let wt_count = proj.cached_worktrees.len().max(1) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(mr_count as u32, mr_count as u32 + wt_count as u32),
            Constraint::Ratio(wt_count as u32, mr_count as u32 + wt_count as u32),
        ])
        .split(area);

    draw_mr_block_render(frame, proj, chunks[0], proj.mr_focused);
    draw_worktree_block_render(frame, proj, chunks[1], !proj.mr_focused);
}

fn draw_mr_block_render(
    frame: &mut Frame,
    proj: &ProjectRenderData<'_>,
    area: Rect,
    focused: bool,
) {
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

fn build_pane_by_path<'a>(panes: &'a [AgentPane]) -> HashMap<PathBuf, &'a AgentPane> {
    panes
        .iter()
        .filter_map(|pane| {
            std::fs::canonicalize(&pane.pane_path)
                .ok()
                .map(|path| (path, pane))
        })
        .collect()
}

fn draw_worktree_block_render(
    frame: &mut Frame,
    proj: &ProjectRenderData<'_>,
    area: Rect,
    focused: bool,
) {
    let border_color = if focused { ACCENT } else { Color::Indexed(238) };
    let wt_count = proj.cached_worktrees.len();

    let block = Block::default()
        .title(Line::from(vec![Span::styled(
            format!(" Worktrees ({}) ", wt_count),
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

    if wt_count == 0 {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  Install worktrunk (wt) for worktree listing",
                Style::default().fg(Color::DarkGray),
            ))),
            section_inner,
        );
        return;
    }

    let pane_by_path = build_pane_by_path(proj.panes);

    let card_h: u16 = 4;
    let total_content = wt_count as u16 * card_h;
    let selected_y = proj.worktree_selected as u16 * card_h;

    let scroll: u16 = if total_content <= section_inner.height {
        0
    } else {
        let max_scroll = total_content.saturating_sub(section_inner.height);
        let ideal = selected_y.saturating_sub(section_inner.height / 2);
        ideal.min(max_scroll)
    };

    for (i, wt) in proj.cached_worktrees.iter().enumerate() {
        let card_y = i as u16 * card_h;
        let sy = card_y as i32 - scroll as i32;
        if sy + card_h as i32 <= 0 || sy >= section_inner.height as i32 {
            continue;
        }
        if sy < 0 || sy as u16 + card_h > section_inner.height {
            continue;
        }
        let ay = section_inner.y + sy as u16;
        let is_selected = focused && i == proj.worktree_selected;
        let rect = Rect::new(section_inner.x, ay, section_inner.width, card_h);
        let matched_pane = wt
            .path
            .as_ref()
            .and_then(|p| std::fs::canonicalize(p).ok())
            .and_then(|canon| pane_by_path.get(&canon).copied());
        render_worktree_card(frame, wt, matched_pane, rect, is_selected);
    }

    if total_content > section_inner.height {
        let mut scrollbar_state = ScrollbarState::new(wt_count).position(proj.worktree_selected);
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
