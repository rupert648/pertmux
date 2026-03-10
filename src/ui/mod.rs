use crate::app::SelectionSection;
use crate::client::ClientState;
use crate::forge_clients::types::{MergeRequestDetail, MergeRequestThread, PipelineJob};
use crate::linking::DashboardState;
use crate::protocol::ProjectSnapshot;
use crate::types::AgentPane;
use crate::worktrunk::WtWorktree;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
};

mod components;
mod helpers;

pub(crate) const ACCENT: Color = Color::Rgb(255, 140, 0);
pub(crate) const NOTIFICATION_DURATION: std::time::Duration = std::time::Duration::from_secs(2);

pub(crate) struct ProjectRenderData<'a> {
    pub(crate) dashboard: &'a DashboardState,
    pub(crate) cached_worktrees: &'a [WtWorktree],
    pub(crate) cached_mr_detail: Option<&'a MergeRequestDetail>,
    pub(crate) cached_pipeline_jobs: &'a [PipelineJob],
    pub(crate) panes: &'a [AgentPane],
    pub(crate) cached_threads: &'a [MergeRequestThread],
    pub(crate) cached_threads_iid: Option<u64>,
    pub(crate) mr_selected: usize,
    pub(crate) worktree_selected: usize,
    pub(crate) mr_focused: bool,
}

impl<'a> ProjectRenderData<'a> {
    pub(crate) fn from_snapshot(
        proj: &'a ProjectSnapshot,
        panes: &'a [AgentPane],
        mr_selected: usize,
        worktree_selected: usize,
        section: &'a SelectionSection,
    ) -> Self {
        Self {
            dashboard: &proj.dashboard,
            cached_worktrees: &proj.cached_worktrees,
            cached_mr_detail: proj.cached_mr_detail.as_ref(),
            cached_pipeline_jobs: &proj.cached_pipeline_jobs,
            panes,
            cached_threads: &proj.cached_threads,
            cached_threads_iid: proj.cached_threads_iid,
            mr_selected,
            worktree_selected,
            mr_focused: matches!(section, SelectionSection::MergeRequests),
        }
    }
}

fn is_landscape(area: Rect) -> bool {
    area.width >= area.height * 2
}

pub fn draw_client(frame: &mut Frame, state: &ClientState) {
    let area = frame.area();

    if is_landscape(area) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);
        components::list_panel::draw_list_panel_client(frame, state, chunks[0]);
        draw_right_panel_client(frame, state, chunks[1]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        components::list_panel::draw_list_panel_client(frame, state, chunks[0]);
        draw_right_panel_client(frame, state, chunks[1]);
    }

    components::notification::draw_notification_client(frame, state, area);
    components::popup::draw_popup_client(frame, state, area);
}

fn draw_right_panel_client(frame: &mut Frame, state: &ClientState, area: Rect) {
    if state.snapshot.projects.len() > 1 {
        let overview_h = (state.snapshot.projects.len() as u16 + 2).min(area.height / 3);
        let chunks =
            Layout::vertical([Constraint::Min(0), Constraint::Length(overview_h)]).split(area);
        components::detail_panel::draw_detail_panel_client(frame, state, chunks[0]);
        components::overview::draw_overview_panel(frame, state, chunks[1]);
    } else {
        components::detail_panel::draw_detail_panel_client(frame, state, area);
    }
}
