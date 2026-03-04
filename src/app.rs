use crate::coding_agent::CodingAgent;
use crate::config::{AgentConfig, Config, ProjectConfig, ProjectSource};
use crate::git::discover_worktrees;
use crate::gitlab::client::GitLabClient;
use crate::gitlab::types::{MergeRequestDetail, MergeRequestSummary, PipelineJob};
use crate::linking::{link_all, DashboardState};
use crate::read_state::ReadStateDb;
use crate::types::{AgentPane, SessionDetail};
use crate::worktrunk::{self, WtWorktree};
use crate::{coding_agent, db, tmux};
use std::time::{Duration, Instant};

pub enum SelectionSection {
    MergeRequests,
    Worktrees,
}

pub enum PopupState {
    None,
    CreateWorktree { input: String },
    ConfirmRemove { branch: String },
    ConfirmMerge { branch: String, worktree_path: String },
}

pub struct ProjectState {
    pub config: ProjectConfig,
    pub client: GitLabClient,
    pub cached_mrs: Vec<MergeRequestSummary>,
    pub cached_mr_detail: Option<MergeRequestDetail>,
    pub cached_pipeline_jobs: Vec<PipelineJob>,
    pub cached_worktrees: Vec<WtWorktree>,
    pub dashboard: DashboardState,
    pub mr_selected: usize,
    pub worktree_selected: usize,
    pub selection_section: SelectionSection,
    pub last_detail_refresh: Instant,
}

pub struct App {
    pub panes: Vec<AgentPane>,
    pub selected: usize,
    pub running: bool,
    pub last_refresh: Instant,
    pub refresh_interval: Duration,
    pub groups: Vec<(String, Vec<usize>)>,
    pub error: Option<String>,
    pub detail: Option<SessionDetail>,
    agent_config: AgentConfig,
    agents: Vec<Box<dyn CodingAgent>>,
    pub projects: Vec<ProjectState>,
    pub active_project: usize,
    pub read_state: Option<ReadStateDb>,
    pub notification: Option<(String, Instant)>,
    pub popup: PopupState,
}

impl App {
    pub fn new(config: Config) -> Self {
        let resolved_projects = config.resolve_projects();
        let gitlab_source = config.gitlab.clone();

        let read_state = if !resolved_projects.is_empty() {
            ReadStateDb::open(None).ok()
        } else {
            None
        };

        let projects: Vec<ProjectState> = resolved_projects
            .into_iter()
            .filter_map(|pc| {
                let client = match pc.source {
                    ProjectSource::Gitlab => {
                        let gl = gitlab_source.as_ref()?;
                        let token = gl.api_token()?;
                        GitLabClient::new(token, &gl.host, &pc.project, pc.username.clone())
                    }
                    ProjectSource::Github => return None,
                };
                Some(ProjectState {
                    config: pc,
                    client,
                    cached_mrs: vec![],
                    cached_mr_detail: None,
                    cached_pipeline_jobs: vec![],
                    cached_worktrees: vec![],
                    dashboard: DashboardState {
                        linked_mrs: vec![],
                    },
                    mr_selected: 0,
                    worktree_selected: 0,
                    selection_section: SelectionSection::MergeRequests,
                    last_detail_refresh: Instant::now() - Duration::from_secs(120),
                })
            })
            .collect();

        let agents = coding_agent::agents_from_config(&config.agent);

        Self {
            panes: Vec::new(),
            selected: 0,
            running: true,
            last_refresh: Instant::now() - Duration::from_secs(10),
            refresh_interval: Duration::from_secs(config.refresh_interval),
            groups: Vec::new(),
            error: None,
            detail: None,
            agent_config: config.agent,
            agents,
            projects,
            active_project: 0,
            read_state,
            notification: None,
            popup: PopupState::None,
        }
    }

    pub fn has_projects(&self) -> bool {
        !self.projects.is_empty()
    }

    pub fn active_project(&self) -> Option<&ProjectState> {
        self.projects.get(self.active_project)
    }

    #[allow(dead_code)]
    pub fn active_project_mut(&mut self) -> Option<&mut ProjectState> {
        self.projects.get_mut(self.active_project)
    }

    pub async fn refresh(&mut self) {
        self.last_refresh = Instant::now();

        let process_names: Vec<&str> = self.agents.iter().map(|a| a.process_name()).collect();

        let mut panes = match tmux::list_agent_panes(&process_names) {
            Ok(p) => {
                if self.error.as_ref().is_some_and(|e| e.starts_with("tmux")) {
                    self.error = None;
                }
                p
            }
            Err(e) => {
                self.error = Some(format!("tmux error: {}", e));
                return;
            }
        };

        for pane in &mut panes {
            if let Some(agent) = self.find_agent(&pane.pane_command) {
                pane.status = agent.query_status(pane.pane_pid);
            }
            let db_path = self
                .agent_config
                .opencode
                .as_ref()
                .and_then(|c| c.db_path.as_deref());
            db::enrich_pane(pane, db_path);
        }

        self.build_groups(&panes);
        self.panes = panes;

        if self.selected >= self.panes.len() && !self.panes.is_empty() {
            self.selected = self.panes.len() - 1;
        }

        self.update_detail();

        let mut link_error: Option<String> = None;
        if let Some(ref read_state) = self.read_state {
            for proj in &mut self.projects {
                match discover_worktrees(&proj.config.local_path).await {
                    Ok(worktrees) => {
                        match link_all(
                            &proj.cached_mrs,
                            &worktrees,
                            &self.panes,
                            read_state,
                            &proj.config.project,
                        ) {
                            Ok(dashboard) => {
                                proj.dashboard = dashboard;
                                if proj.mr_selected >= proj.dashboard.linked_mrs.len()
                                    && !proj.dashboard.linked_mrs.is_empty()
                                {
                                    proj.mr_selected = proj.dashboard.linked_mrs.len() - 1;
                                }
                            }
                            Err(e) => {
                                link_error = Some(format!("Linking error: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        link_error = Some(format!("Worktree discovery error: {}", e));
                    }
                }
            }
        }
        if let Some(e) = link_error {
            self.error = Some(e);
        }
    }

    fn build_groups(&mut self, panes: &[AgentPane]) {
        use std::collections::BTreeMap;
        let mut map: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, pane) in panes.iter().enumerate() {
            map.entry(pane.session_name.clone()).or_default().push(i);
        }
        self.groups = map.into_iter().collect();
    }

    pub fn move_up(&mut self) {
        if let Some(proj) = self.projects.get_mut(self.active_project) {
            match proj.selection_section {
                SelectionSection::MergeRequests => {
                    if proj.mr_selected > 0 {
                        proj.mr_selected -= 1;
                    }
                }
                SelectionSection::Worktrees => {
                    if proj.worktree_selected > 0 {
                        proj.worktree_selected -= 1;
                    }
                }
            }
        } else if !self.panes.is_empty() && self.selected > 0 {
            self.selected -= 1;
            self.update_detail();
        }
    }

    pub fn move_down(&mut self) {
        if let Some(proj) = self.projects.get_mut(self.active_project) {
            match proj.selection_section {
                SelectionSection::MergeRequests => {
                    if !proj.dashboard.linked_mrs.is_empty()
                        && proj.mr_selected < proj.dashboard.linked_mrs.len() - 1
                    {
                        proj.mr_selected += 1;
                    }
                }
                SelectionSection::Worktrees => {
                    if !proj.cached_worktrees.is_empty()
                        && proj.worktree_selected < proj.cached_worktrees.len() - 1
                    {
                        proj.worktree_selected += 1;
                    }
                }
            }
        } else if !self.panes.is_empty() && self.selected < self.panes.len() - 1 {
            self.selected += 1;
            self.update_detail();
        }
    }

    pub fn toggle_section(&mut self) {
        if let Some(proj) = self.projects.get_mut(self.active_project) {
            proj.selection_section = match proj.selection_section {
                SelectionSection::MergeRequests => SelectionSection::Worktrees,
                SelectionSection::Worktrees => SelectionSection::MergeRequests,
            };
        }
    }

    pub fn next_project(&mut self) {
        if !self.projects.is_empty() && self.active_project < self.projects.len() - 1 {
            self.active_project += 1;
        }
    }

    pub fn prev_project(&mut self) {
        if self.active_project > 0 {
            self.active_project -= 1;
        }
    }

    pub fn focus_selected(&self) -> anyhow::Result<()> {
        if let Some(proj) = self.projects.get(self.active_project) {
            match proj.selection_section {
                SelectionSection::MergeRequests => {
                    if let Some(linked) = proj.dashboard.linked_mrs.get(proj.mr_selected)
                        && let Some(pane) = linked.tmux_pane.as_ref()
                    {
                        tmux::switch_to_pane(&pane.pane_id)?;
                    }
                }
                SelectionSection::Worktrees => {
                    if let Some(wt) = proj.cached_worktrees.get(proj.worktree_selected)
                        && let Some(ref path) = wt.path
                    {
                        tmux::find_or_create_pane(path)?;
                    }
                }
            }
        } else if let Some(pane) = self.panes.get(self.selected) {
            tmux::switch_to_pane(&pane.pane_id)?;
        }
        Ok(())
    }

    pub async fn refresh_mrs(&mut self) {
        let mut last_error: Option<String> = None;
        let mut had_success = false;

        for proj in &mut self.projects {
            match proj.client.fetch_mr_list().await {
                Ok(mrs) => {
                    proj.cached_mrs = mrs;
                    had_success = true;
                }
                Err(e) => {
                    last_error =
                        Some(format!("GitLab error ({}): {}", proj.config.name, e));
                }
            }
        }

        if had_success && self.error.as_ref().is_some_and(|e| e.starts_with("GitLab")) {
            self.error = None;
        }
        if let Some(e) = last_error {
            self.error = Some(e);
        }
    }

    pub async fn refresh_mr_detail(&mut self) {
        let Some(proj) = self.projects.get_mut(self.active_project) else {
            return;
        };

        let Some(linked_mr) = proj.dashboard.linked_mrs.get(proj.mr_selected) else {
            return;
        };

        let iid = linked_mr.mr.iid;

        match proj.client.fetch_mr_detail(iid).await {
            Ok(detail) => {
                proj.cached_mr_detail = Some(detail);
                proj.last_detail_refresh = Instant::now();
            }
            Err(e) => {
                self.error = Some(format!("MR detail error: {}", e));
                return;
            }
        }

        // API returns id desc — reverse to get stage order
        let pipeline_id = proj
            .cached_mr_detail
            .as_ref()
            .and_then(|d| d.head_pipeline.as_ref())
            .map(|p| p.id);

        if let Some(pid) = pipeline_id {
            match proj.client.fetch_pipeline_jobs(pid).await {
                Ok(mut jobs) => {
                    jobs.reverse();
                    proj.cached_pipeline_jobs = jobs;
                }
                Err(_) => {
                    proj.cached_pipeline_jobs = vec![];
                }
            }
        } else {
            proj.cached_pipeline_jobs = vec![];
        }

        if let Some(ref read_state) = self.read_state {
            if let Ok(notes) = proj.client.fetch_mr_notes(iid).await {
                let note_ids: Vec<u64> = notes.iter().map(|n| n.id).collect();
                let _ = read_state.mark_notes_seen(&proj.config.project, iid, &note_ids);
                let _ = read_state.mark_mr_viewed(&proj.config.project, iid, notes.len() as u32);
            }
        }
    }

    pub async fn refresh_worktrees(&mut self) {
        for proj in &mut self.projects {
            match worktrunk::fetch_worktrees(&proj.config.local_path).await {
                Ok(wts) => {
                    proj.cached_worktrees = wts;
                    if proj.worktree_selected >= proj.cached_worktrees.len()
                        && !proj.cached_worktrees.is_empty()
                    {
                        proj.worktree_selected = proj.cached_worktrees.len() - 1;
                    }
                }
                Err(e) => {
                    eprintln!("[pertmux] worktrunk error ({}): {}", proj.config.name, e);
                    proj.cached_worktrees = vec![];
                }
            }
        }
    }

    pub fn seconds_since_refresh(&self) -> u64 {
        self.last_refresh.elapsed().as_secs()
    }

    fn update_detail(&mut self) {
        self.detail = self
            .panes
            .get(self.selected)
            .and_then(|pane| pane.db_session_id.as_deref())
            .and_then(|id| {
                let db_path = self
                    .agent_config
                    .opencode
                    .as_ref()
                    .and_then(|c| c.db_path.as_deref());
                db::fetch_session_detail(id, db_path)
            });
    }

    pub fn copy_selected_branch(&mut self) {
        let branch = if let Some(proj) = self.projects.get(self.active_project) {
            match proj.selection_section {
                SelectionSection::MergeRequests => proj
                    .dashboard
                    .linked_mrs
                    .get(proj.mr_selected)
                    .map(|l| l.mr.source_branch.clone()),
                SelectionSection::Worktrees => proj
                    .cached_worktrees
                    .get(proj.worktree_selected)
                    .and_then(|wt| wt.branch.clone()),
            }
        } else {
            None
        };
        if let Some(branch) = branch {
            let ok = std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    use std::io::Write;
                    if let Some(ref mut stdin) = child.stdin {
                        stdin.write_all(branch.as_bytes())?;
                    }
                    child.wait()
                })
                .is_ok();
            if ok {
                self.notification = Some((format!("Copied: {}", branch), Instant::now()));
            }
        }
    }

    pub fn open_selected_mr_in_browser(&self) {
        if let Some(proj) = self.projects.get(self.active_project)
            && let Some(linked) = proj.dashboard.linked_mrs.get(proj.mr_selected)
        {
            let url = &linked.mr.web_url;
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(url).spawn();
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            let _ = std::process::Command::new("open").arg(url).spawn();
        }
    }

    pub fn has_popup(&self) -> bool {
        !matches!(self.popup, PopupState::None)
    }

    pub fn open_create_popup(&mut self) {
        if let Some(proj) = self.projects.get(self.active_project) {
            if matches!(proj.selection_section, SelectionSection::Worktrees) {
                self.popup = PopupState::CreateWorktree {
                    input: String::new(),
                };
            }
        }
    }

    pub fn open_remove_popup(&mut self) {
        if let Some(proj) = self.projects.get(self.active_project) {
            if matches!(proj.selection_section, SelectionSection::Worktrees) {
                if let Some(wt) = proj.cached_worktrees.get(proj.worktree_selected) {
                    if wt.is_main {
                        self.notification =
                            Some(("Cannot remove main worktree".into(), Instant::now()));
                        return;
                    }
                    if let Some(ref branch) = wt.branch {
                        self.popup = PopupState::ConfirmRemove {
                            branch: branch.clone(),
                        };
                    }
                }
            }
        }
    }

    pub fn open_merge_popup(&mut self) {
        if let Some(proj) = self.projects.get(self.active_project) {
            if matches!(proj.selection_section, SelectionSection::Worktrees) {
                if let Some(wt) = proj.cached_worktrees.get(proj.worktree_selected) {
                    if wt.is_main {
                        self.notification =
                            Some(("Cannot merge main worktree".into(), Instant::now()));
                        return;
                    }
                    if let (Some(branch), Some(path)) = (&wt.branch, &wt.path) {
                        self.popup = PopupState::ConfirmMerge {
                            branch: branch.clone(),
                            worktree_path: path.clone(),
                        };
                    }
                }
            }
        }
    }

    pub fn close_popup(&mut self) {
        self.popup = PopupState::None;
    }

    pub fn popup_input_push(&mut self, ch: char) {
        if let PopupState::CreateWorktree { ref mut input } = self.popup {
            input.push(ch);
        }
    }

    pub fn popup_input_pop(&mut self) {
        if let PopupState::CreateWorktree { ref mut input } = self.popup {
            input.pop();
        }
    }

    pub async fn confirm_popup_action(&mut self) {
        let popup = std::mem::replace(&mut self.popup, PopupState::None);
        let local_path = self
            .projects
            .get(self.active_project)
            .map(|p| p.config.local_path.clone());

        match popup {
            PopupState::CreateWorktree { ref input } => {
                let branch = input.trim().to_string();
                if branch.is_empty() {
                    return;
                }
                if let Some(ref lp) = local_path {
                    match worktrunk::create_worktree(lp, &branch).await {
                        Ok(msg) => {
                            self.notification = Some((msg, Instant::now()));
                            self.refresh_worktrees().await;
                        }
                        Err(e) => {
                            self.notification =
                                Some((format!("Create failed: {}", e), Instant::now()));
                        }
                    }
                }
            }
            PopupState::ConfirmRemove { ref branch } => {
                if let Some(ref lp) = local_path {
                    match worktrunk::remove_worktree(lp, branch).await {
                        Ok(msg) => {
                            self.notification = Some((msg, Instant::now()));
                            self.refresh_worktrees().await;
                        }
                        Err(e) => {
                            self.notification =
                                Some((format!("Remove failed: {}", e), Instant::now()));
                        }
                    }
                }
            }
            PopupState::ConfirmMerge {
                ref branch,
                ref worktree_path,
            } => match worktrunk::merge_worktree(worktree_path).await {
                Ok(_) => {
                    self.notification = Some((
                        format!("Merged {} into default branch", branch),
                        Instant::now(),
                    ));
                    self.refresh_worktrees().await;
                }
                Err(e) => {
                    self.notification = Some((format!("Merge failed: {}", e), Instant::now()));
                }
            },
            PopupState::None => {}
        }
    }

    fn find_agent(&self, command: &str) -> Option<&dyn CodingAgent> {
        self.agents
            .iter()
            .find(|a| a.process_name() == command)
            .map(|a| a.as_ref())
    }
}
