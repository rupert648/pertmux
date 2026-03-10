use crate::coding_agent::CodingAgent;
use crate::config::{AgentConfig, Config, ProjectConfig, ProjectForge};
use crate::forge_clients::traits::ForgeClient;
use crate::forge_clients::types::{
    MergeRequestDetail, MergeRequestSummary, MergeRequestThread, PipelineJob,
};
use crate::forge_clients::{GitHubClient, GitLabClient};
use crate::git::discover_worktrees;
use crate::linking::{DashboardState, link_all};
use crate::protocol::{DashboardSnapshot, ProjectSnapshot};
use crate::read_state::ReadStateDb;
use crate::tmux;
use crate::types::{AgentPane, SessionDetail};
use crate::worktrunk::{self, WtWorktree};
use crate::{coding_agent, db};
use std::time::{Duration, Instant};

pub enum SelectionSection {
    MergeRequests,
    Worktrees,
}

pub enum PopupState {
    None,
    CreateWorktree {
        input: String,
    },
    ConfirmRemove {
        branch: String,
    },
    ConfirmMerge {
        branch: String,
        worktree_path: String,
    },
    ProjectFilter {
        input: String,
        filtered: Vec<(usize, String)>,
        selected: usize,
    },
}

pub struct ProjectState {
    pub config: ProjectConfig,
    pub client: Box<dyn ForgeClient>,
    pub cached_mrs: Vec<MergeRequestSummary>,
    pub cached_mr_detail: Option<MergeRequestDetail>,
    pub cached_pipeline_jobs: Vec<PipelineJob>,
    pub cached_threads: Vec<MergeRequestThread>,
    pub cached_threads_iid: Option<u64>,
    pub cached_worktrees: Vec<WtWorktree>,
    pub dashboard: DashboardState,
    pub mr_selected: usize,
    pub worktree_selected: usize,
    #[allow(dead_code)]
    pub selection_section: SelectionSection,
    pub last_detail_refresh: Instant,
}

pub struct App {
    pub panes: Vec<AgentPane>,
    pub selected: usize,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub notification: Option<(String, Instant)>,
    #[allow(dead_code)]
    pub popup: PopupState,
}

impl App {
    pub fn new(config: Config) -> Self {
        let resolved_projects = config.resolve_projects();
        let gitlab_source = config.gitlab.clone();
        let github_source = config.github.clone();

        let read_state = if !resolved_projects.is_empty() {
            ReadStateDb::open(None).ok()
        } else {
            None
        };

        let projects: Vec<ProjectState> = resolved_projects
            .into_iter()
            .filter_map(|pc| {
                let client: Box<dyn ForgeClient> = match pc.source {
                    ProjectForge::Gitlab => {
                        let gl = gitlab_source.as_ref()?;
                        let token = gl.api_token()?;
                        Box::new(GitLabClient::new(
                            token,
                            &gl.host,
                            &pc.project,
                            pc.username.clone(),
                        ))
                    }
                    ProjectForge::Github => {
                        let gh = github_source.as_ref()?;
                        let token = gh.api_token()?;
                        Box::new(GitHubClient::new(
                            token,
                            &gh.host,
                            &pc.project,
                            pc.username.clone(),
                        ))
                    }
                };
                Some(ProjectState {
                    config: pc,
                    client,
                    cached_mrs: vec![],
                    cached_mr_detail: None,
                    cached_pipeline_jobs: vec![],
                    cached_threads: vec![],
                    cached_threads_iid: None,
                    cached_worktrees: vec![],
                    dashboard: DashboardState { linked_mrs: vec![] },
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

    pub async fn refresh_mrs(&mut self) {
        let mut last_error: Option<String> = None;
        let mut had_success = false;

        for proj in &mut self.projects {
            match proj.client.fetch_mrs().await {
                Ok(mrs) => {
                    proj.cached_mrs = mrs;
                    had_success = true;
                }
                Err(e) => {
                    last_error = Some(format!("Forge error ({}): {}", proj.config.name, e));
                }
            }
        }

        if had_success
            && self
                .error
                .as_ref()
                .is_some_and(|e| e.starts_with("Forge") || e.starts_with("GitLab"))
        {
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

        match proj
            .client
            .fetch_ci_jobs(proj.cached_mr_detail.as_ref().unwrap())
            .await
        {
            Ok(mut jobs) => {
                jobs.reverse();
                proj.cached_pipeline_jobs = jobs;
            }
            Err(_) => {
                proj.cached_pipeline_jobs = vec![];
            }
        }

        match proj.client.fetch_discussions(iid).await {
            Ok(threads) => {
                proj.cached_threads = threads;
                proj.cached_threads_iid = Some(iid);
            }
            Err(_) => {
                // Preserve existing cached threads on fetch error
            }
        }

        if let Some(ref read_state) = self.read_state
            && let Ok(notes) = proj.client.fetch_notes(iid).await
        {
            let note_ids: Vec<u64> = notes.iter().map(|n| n.id).collect();
            let _ = read_state.mark_notes_seen(&proj.config.project, iid, &note_ids);
            let _ = read_state.mark_mr_viewed(&proj.config.project, iid, notes.len() as u32);
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

    pub fn snapshot(&self) -> DashboardSnapshot {
        DashboardSnapshot {
            projects: self
                .projects
                .iter()
                .map(|p| ProjectSnapshot {
                    name: p.config.name.clone(),
                    source: p.config.source.clone(),
                    project_path: p.config.project.clone(),
                    local_path: p.config.local_path.clone(),
                    dashboard: p.dashboard.clone(),
                    cached_worktrees: p.cached_worktrees.clone(),
                    cached_mr_detail: p.cached_mr_detail.clone(),
                    cached_pipeline_jobs: p.cached_pipeline_jobs.clone(),
                    cached_threads: p.cached_threads.clone(),
                    cached_threads_iid: p.cached_threads_iid,
                })
                .collect(),
            panes: self.panes.clone(),
            groups: self.groups.clone(),
            detail: self.detail.clone(),
            error: self.error.clone(),
            seconds_since_refresh: self.seconds_since_refresh(),
        }
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

    fn find_agent(&self, command: &str) -> Option<&dyn CodingAgent> {
        self.agents
            .iter()
            .find(|a| a.process_name() == command)
            .map(|a| a.as_ref())
    }
}
