use crate::agent_changes::{AgentChange, AgentChangeType};
use crate::coding_agent;
use crate::coding_agent::CodingAgent;
use crate::config::{AgentActionConfig, Config, KeybindingsConfig, ProjectConfig, ProjectForge};
use crate::forge_clients::traits::ForgeClient;
use crate::forge_clients::types::{
    MergeRequestDetail, MergeRequestSummary, MergeRequestThread, PipelineJob,
};
use crate::forge_clients::{GitHubClient, GitLabClient};
use crate::git::discover_worktrees;
use crate::linking::{DashboardState, link_all};
use crate::mr_changes::{MrChange, MrChangeType};
use crate::protocol::{
    ActivityEntry, DaemonMsg, DashboardSnapshot, GlobalMrEntry, ProjectSnapshot, RefreshStep,
};
use crate::read_state::ReadStateDb;
use crate::tmux;
use crate::types::{AgentPane, PaneStatus, SessionDetail};
use crate::worktrunk::{self, WtWorktree};
use futures::StreamExt as _;
use jiff::Timestamp as JiffTimestamp;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

pub enum SelectionSection {
    MergeRequests,
    Worktrees,
}

pub enum PopupState {
    None,
    CreateWorktree {
        input: String,
    },
    /// Two-field popup: enter a branch name and a message to inject into the
    /// `default_worktree_with_prompt` command template (`{{msg}}` placeholder).
    CreateWorktreeWithPrompt {
        branch_input: String,
        prompt_input: String,
        /// 0 = branch field focused, 1 = prompt field focused.
        focused_field: usize,
    },
    ConfirmRemove {
        branch: String,
        /// Pane ID of the tmux window at the worktree path, captured before deletion.
        /// `None` when no tmux window is open at that path.
        linked_pane_id: Option<String>,
    },
    /// After a successful worktree removal, ask whether to also kill the linked tmux window.
    ConfirmKillTmuxWindow {
        branch: String,
        pane_id: String,
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
    ChangeSummary {
        changes: Vec<MrChange>,
        selected: usize,
    },
    AgentActions {
        selected: usize,
        pane_pid: u32,
        session_id: String,
        worktree_branch: Option<String>,
    },
    MrOverview {
        selected: usize,
    },
    ActivityFeed {
        selected: usize,
    },
    KeybindingsHelp,
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
    pub mr_detail_interval: Duration,
    pub worktree_interval: Duration,
    pub mr_list_interval: Duration,
    pub groups: Vec<(String, Vec<usize>)>,
    pub error: Option<String>,
    pub detail: Option<SessionDetail>,
    agents: Vec<Box<dyn CodingAgent>>,
    pub projects: Vec<ProjectState>,
    pub active_project: usize,
    pub read_state: Option<ReadStateDb>,
    pub default_agent_command: Option<String>,
    /// Command template for "create worktree with prompt". Contains `{{msg}}` placeholder.
    pub default_worktree_with_prompt: Option<String>,
    #[allow(dead_code)]
    pub notification: Option<(String, Instant)>,
    #[allow(dead_code)]
    pub popup: PopupState,
    pub keybindings: KeybindingsConfig,
    pub pending_changes: Vec<MrChange>,
    pub pending_agent_changes: Vec<AgentChange>,
    previous_pane_statuses: HashMap<String, PaneStatus>,
    pub agent_actions: Vec<AgentActionConfig>,
    pub global_mrs: Vec<GlobalMrEntry>,
    /// Full activity feed history, accumulated by the daemon and included in
    /// every snapshot so clients see history even after reconnecting.
    pub activity_feed: VecDeque<ActivityEntry>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let resolved_projects = config.resolve_projects();
        let gitlab_source = config.gitlab.clone();
        let github_source = config.github.clone();
        let default_agent_command = config.default_agent_command.clone();
        let keybindings = config.keybindings.clone();

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
                    selection_section: SelectionSection::Worktrees,
                    last_detail_refresh: Instant::now() - Duration::from_secs(120),
                })
            })
            .collect();

        let agents = coding_agent::agents_from_config(&config.agent);
        let default_worktree_with_prompt = config.default_worktree_with_prompt.clone();

        Self {
            panes: Vec::new(),
            selected: 0,
            running: true,
            last_refresh: Instant::now() - Duration::from_secs(10),
            refresh_interval: Duration::from_secs(config.refresh_interval),
            mr_detail_interval: Duration::from_secs(config.mr_detail_interval),
            worktree_interval: Duration::from_secs(config.worktree_interval),
            mr_list_interval: Duration::from_secs(config.mr_list_interval),
            groups: Vec::new(),
            error: None,
            detail: None,
            agents,
            projects,
            active_project: 0,
            read_state,
            default_agent_command,
            default_worktree_with_prompt,
            notification: None,
            popup: PopupState::None,
            keybindings,
            pending_changes: Vec::new(),
            pending_agent_changes: Vec::new(),
            previous_pane_statuses: HashMap::new(),
            agent_actions: config.agent_action,
            global_mrs: Vec::new(),
            activity_feed: VecDeque::new(),
        }
    }

    pub fn has_projects(&self) -> bool {
        !self.projects.is_empty()
    }

    pub async fn refresh(&mut self) {
        info!("app::refresh: start");
        let t = std::time::Instant::now();
        self.last_refresh = Instant::now();
        self.error = None;

        let process_names: Vec<&str> = self.agents.iter().map(|a| a.process_name()).collect();

        let mut panes = match tmux::list_agent_panes(&process_names) {
            Ok(p) => p,
            Err(e) => {
                warn!("app::refresh: tmux error after {:.2?}: {}", t.elapsed(), e);
                self.error = Some(format!("tmux error: {}", e));
                return;
            }
        };
        info!(
            "app::refresh: tmux listed {} panes in {:.2?}",
            panes.len(),
            t.elapsed()
        );

        for pane in &mut panes {
            if let Some(agent) = self.find_agent(&pane.pane_command) {
                pane.status = agent.query_status(pane);
                agent.enrich_pane(pane);
            }
        }

        let prev_changed_at: HashMap<String, Option<JiffTimestamp>> = self
            .panes
            .iter()
            .map(|p| (p.pane_id.clone(), p.status_changed_at))
            .collect();

        for pane in &mut panes {
            let prev_status = self.previous_pane_statuses.get(&pane.pane_id);
            match prev_status {
                None => {}
                Some(prev) if statuses_match(prev, &pane.status) => {
                    if let Some(prev_ts) = prev_changed_at.get(&pane.pane_id) {
                        pane.status_changed_at = *prev_ts;
                    }
                }
                Some(prev) => {
                    pane.status_changed_at = Some(JiffTimestamp::now());
                    if let Some(change_type) = agent_change_type(prev, &pane.status) {
                        let agent_change = AgentChange {
                            pane_id: pane.pane_id.clone(),
                            pane_path: pane.pane_path.clone(),
                            session_name: pane.session_name.clone(),
                            change_type,
                        };
                        self.activity_feed
                            .push_front(ActivityEntry::from(&agent_change));
                        self.pending_agent_changes.push(agent_change);
                        self.activity_feed.truncate(50);
                    }
                }
            }

            self.previous_pane_statuses
                .insert(pane.pane_id.clone(), pane.status.clone());
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
                info!(
                    "app::refresh: discover_worktrees for {} (path={})",
                    proj.config.name, proj.config.local_path
                );
                let dt = std::time::Instant::now();
                match discover_worktrees(&proj.config.local_path).await {
                    Ok(worktrees) => {
                        info!(
                            "app::refresh: discovered {} worktrees in {:.2?}",
                            worktrees.len(),
                            dt.elapsed()
                        );
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
                        warn!(
                            "app::refresh: discover_worktrees failed after {:.2?}: {}",
                            dt.elapsed(),
                            e
                        );
                        link_error = Some(format!("Worktree discovery error: {}", e));
                    }
                }
            }
        }
        if let Some(e) = link_error {
            self.error = Some(e);
        }
        info!("app::refresh: done in {:.2?}", t.elapsed());
    }

    fn build_groups(&mut self, panes: &[AgentPane]) {
        use std::collections::BTreeMap;
        let mut map: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, pane) in panes.iter().enumerate() {
            map.entry(pane.session_name.clone()).or_default().push(i);
        }
        self.groups = map.into_iter().collect();
    }

    pub async fn refresh_mrs(&mut self, progress_tx: Option<&broadcast::Sender<DaemonMsg>>) {
        let total = self.projects.len();
        info!("app::refresh_mrs: start ({} projects, parallel)", total);
        let t = std::time::Instant::now();

        // FuturesUnordered lets us process each project's result as soon as it
        // arrives and emit a progress broadcast, rather than waiting for all.
        let mut stream = futures::stream::FuturesUnordered::new();
        for (i, proj) in self.projects.iter().enumerate() {
            let fut = proj.client.fetch_mrs();
            stream.push(async move { (i, fut.await) });
        }

        let mut done = 0;
        let mut indexed: Vec<
            Option<anyhow::Result<Vec<crate::forge_clients::types::MergeRequestSummary>>>,
        > = (0..total).map(|_| None).collect();

        while let Some((i, result)) = stream.next().await {
            done += 1;
            indexed[i] = Some(result);
            if let Some(tx) = progress_tx {
                let _ = tx.send(DaemonMsg::Progress(vec![RefreshStep {
                    label: "Updating MRs".into(),
                    done,
                    total,
                }]));
                // Yield so handle_client tasks can forward the broadcast to clients.
                tokio::task::yield_now().await;
            }
        }
        drop(stream); // release borrows on self.projects before mutable iteration

        let mut last_error: Option<String> = None;
        for (proj, result) in self.projects.iter_mut().zip(indexed) {
            match result.unwrap_or_else(|| Err(anyhow::anyhow!("missing"))) {
                Ok(mrs) => {
                    info!(
                        "app::refresh_mrs: got {} MRs for {}",
                        mrs.len(),
                        proj.config.name
                    );
                    if !proj.cached_mrs.is_empty() {
                        let changes =
                            detect_mr_list_changes(&proj.config.name, &proj.cached_mrs, &mrs);
                        for c in &changes {
                            self.activity_feed.push_front(ActivityEntry::from(c));
                        }
                        self.activity_feed.truncate(50);
                        self.pending_changes.extend(changes);
                    }
                    proj.cached_mrs = mrs;
                }
                Err(e) => {
                    warn!("app::refresh_mrs: error for {}: {}", proj.config.name, e);
                    last_error = Some(format!("Forge error ({}): {}", proj.config.name, e));
                }
            }
        }
        if let Some(e) = last_error {
            self.error = Some(e);
        }
        info!("app::refresh_mrs: done in {:.2?}", t.elapsed());
    }

    pub async fn refresh_global_mrs(&mut self, progress_tx: Option<&broadcast::Sender<DaemonMsg>>) {
        info!("app::refresh_global_mrs: start");
        let t = std::time::Instant::now();

        // Collect (index, forge) pairs so we can drive all fetches in parallel.
        let gl_indices: Vec<usize> = self
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.config.source, ProjectForge::Gitlab))
            .map(|(i, _)| i)
            .collect();
        let gh_indices: Vec<usize> = self
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.config.source, ProjectForge::Github))
            .map(|(i, _)| i)
            .collect();

        let total = gl_indices.len() + gh_indices.len();
        let mut done = 0;

        // GitLab — parallel fetch_mrs per project
        let mut gl_stream = futures::stream::FuturesUnordered::new();
        for &i in &gl_indices {
            let proj = &self.projects[i];
            let name = proj.config.name.clone();
            let project_path = proj.config.project.clone();
            let fut = proj.client.fetch_mrs();
            gl_stream.push(async move { (name, project_path, fut.await) });
        }

        let mut gl_results: Vec<(
            String,
            String,
            anyhow::Result<Vec<crate::forge_clients::types::MergeRequestSummary>>,
        )> = Vec::new();
        while let Some(res) = gl_stream.next().await {
            done += 1;
            gl_results.push(res);
            if let Some(tx) = progress_tx {
                let _ = tx.send(DaemonMsg::Progress(vec![RefreshStep {
                    label: "Global feed".into(),
                    done,
                    total,
                }]));
                tokio::task::yield_now().await;
            }
        }
        drop(gl_stream);

        // GitHub — parallel fetch_user_mrs per project
        let mut gh_stream = futures::stream::FuturesUnordered::new();
        for &i in &gh_indices {
            let proj = &self.projects[i];
            let name = proj.config.name.clone();
            let fut = proj.client.fetch_user_mrs();
            gh_stream.push(async move { (name, fut.await) });
        }

        let mut gh_results: Vec<(
            String,
            anyhow::Result<Vec<crate::forge_clients::types::UserMrSummary>>,
        )> = Vec::new();
        while let Some(res) = gh_stream.next().await {
            done += 1;
            gh_results.push(res);
            if let Some(tx) = progress_tx {
                let _ = tx.send(DaemonMsg::Progress(vec![RefreshStep {
                    label: "Global feed".into(),
                    done,
                    total,
                }]));
                tokio::task::yield_now().await;
            }
        }
        drop(gh_stream);

        // Assemble entries from all results
        let mut all_entries: Vec<GlobalMrEntry> = Vec::new();

        for (name, project_path, result) in gl_results {
            match result {
                Ok(mrs) => {
                    all_entries.extend(mrs.into_iter().map(|mr| GlobalMrEntry {
                        forge: ProjectForge::Gitlab,
                        configured_project: Some(name.clone()),
                        mr: crate::forge_clients::types::UserMrSummary {
                            iid: mr.iid,
                            title: mr.title,
                            web_url: mr.web_url.clone(),
                            project_path: project_path.clone(),
                            author: mr.author,
                            draft: mr.draft,
                            updated_at: mr.updated_at,
                        },
                    }));
                }
                Err(e) => error!("global GitLab MR fetch error ({}): {}", name, e),
            }
        }

        for (name, result) in gh_results {
            match result {
                Ok(mrs) => {
                    all_entries.extend(mrs.into_iter().map(|mr| GlobalMrEntry {
                        forge: ProjectForge::Github,
                        configured_project: Some(name.clone()),
                        mr,
                    }));
                }
                Err(e) => error!("global GitHub MR fetch error ({}): {}", name, e),
            }
        }

        all_entries.sort_by(|a, b| {
            a.mr.project_path
                .cmp(&b.mr.project_path)
                .then_with(|| b.mr.updated_at.cmp(&a.mr.updated_at))
        });

        info!(
            "app::refresh_global_mrs: done — {} entries in {:.2?}",
            all_entries.len(),
            t.elapsed()
        );
        self.global_mrs = all_entries;
    }

    pub async fn refresh_mr_detail(&mut self) {
        let Some(proj) = self.projects.get_mut(self.active_project) else {
            return;
        };

        let Some(linked_mr) = proj.dashboard.linked_mrs.get(proj.mr_selected) else {
            return;
        };

        let iid = linked_mr.mr.iid;
        info!(
            "app::refresh_mr_detail: start (project={}, iid={})",
            proj.config.name, iid
        );
        let t = std::time::Instant::now();

        let project_name = proj.config.name.clone();
        info!("app::refresh_mr_detail: fetching detail for iid={}", iid);
        let st = std::time::Instant::now();
        match proj.client.fetch_mr_detail(iid).await {
            Ok(detail) => {
                info!("app::refresh_mr_detail: got detail in {:.2?}", st.elapsed());
                if let Some(ref old_detail) = proj.cached_mr_detail {
                    let changes = detect_mr_detail_changes(&project_name, old_detail, &detail);
                    for c in &changes {
                        self.activity_feed.push_front(ActivityEntry::from(c));
                    }
                    self.activity_feed.truncate(50);
                    self.pending_changes.extend(changes);
                }
                proj.cached_mr_detail = Some(detail);
                proj.last_detail_refresh = Instant::now();
            }
            Err(e) => {
                warn!(
                    "app::refresh_mr_detail: fetch_mr_detail error after {:.2?}: {}",
                    st.elapsed(),
                    e
                );
                self.error = Some(format!("MR detail error: {}", e));
                return;
            }
        }

        info!("app::refresh_mr_detail: fetching CI jobs");
        let st = std::time::Instant::now();
        match proj
            .client
            .fetch_ci_jobs(proj.cached_mr_detail.as_ref().unwrap())
            .await
        {
            Ok(mut jobs) => {
                info!(
                    "app::refresh_mr_detail: got {} CI jobs in {:.2?}",
                    jobs.len(),
                    st.elapsed()
                );
                jobs.reverse();
                proj.cached_pipeline_jobs = jobs;
            }
            Err(e) => {
                warn!(
                    "app::refresh_mr_detail: fetch_ci_jobs error after {:.2?}: {}",
                    st.elapsed(),
                    e
                );
                proj.cached_pipeline_jobs = vec![];
            }
        }

        info!("app::refresh_mr_detail: fetching discussions");
        let st = std::time::Instant::now();
        match proj.client.fetch_discussions(iid).await {
            Ok(threads) => {
                info!(
                    "app::refresh_mr_detail: got {} discussion threads in {:.2?}",
                    threads.len(),
                    st.elapsed()
                );
                proj.cached_threads = threads;
                proj.cached_threads_iid = Some(iid);
            }
            Err(e) => {
                warn!(
                    "app::refresh_mr_detail: fetch_discussions error after {:.2?}: {}",
                    st.elapsed(),
                    e
                );
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
        info!("app::refresh_mr_detail: done in {:.2?}", t.elapsed());
    }

    /// Refresh worktrees for all projects in parallel.
    pub async fn refresh_worktrees(&mut self, progress_tx: Option<&broadcast::Sender<DaemonMsg>>) {
        info!(
            "app::refresh_worktrees: start ({} projects, parallel)",
            self.projects.len()
        );
        let t = std::time::Instant::now();

        // Collect (index, name, path) so all wt calls run concurrently with no
        // borrows on self.projects across await points.
        let entries: Vec<(usize, String, String)> = self
            .projects
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.config.name.clone(), p.config.local_path.clone()))
            .collect();
        let total = entries.len();

        let mut stream = futures::stream::FuturesUnordered::new();
        for (i, name, path) in &entries {
            let i = *i;
            let name = name.clone();
            let path = path.clone();
            stream.push(async move {
                info!(
                    "app::refresh_worktrees: fetching for {} (path={})",
                    name, path
                );
                let pt = std::time::Instant::now();
                let result = worktrunk::fetch_worktrees(&path).await;
                (i, name, pt.elapsed(), result)
            });
        }

        let mut done = 0;
        let mut indexed: Vec<
            Option<(
                std::time::Duration,
                anyhow::Result<Vec<crate::worktrunk::WtWorktree>>,
            )>,
        > = (0..total).map(|_| None).collect();

        while let Some((i, _name, elapsed, result)) = stream.next().await {
            done += 1;
            indexed[i] = Some((elapsed, result));
            if let Some(tx) = progress_tx {
                let _ = tx.send(DaemonMsg::Progress(vec![RefreshStep {
                    label: "Worktrees".into(),
                    done,
                    total,
                }]));
                tokio::task::yield_now().await;
            }
        }
        drop(stream);

        for (proj, entry) in self.projects.iter_mut().zip(indexed) {
            let (elapsed, result) = entry
                .unwrap_or_else(|| (std::time::Duration::ZERO, Err(anyhow::anyhow!("missing"))));
            match result {
                Ok(wts) => {
                    info!(
                        "app::refresh_worktrees: got {} worktrees for {} in {:.2?}",
                        wts.len(),
                        proj.config.name,
                        elapsed
                    );
                    proj.cached_worktrees = wts;
                    if proj.worktree_selected >= proj.cached_worktrees.len()
                        && !proj.cached_worktrees.is_empty()
                    {
                        proj.worktree_selected = proj.cached_worktrees.len() - 1;
                    }
                }
                Err(e) => {
                    warn!(
                        "app::refresh_worktrees: error for {} after {:.2?}: {}",
                        proj.config.name, elapsed, e
                    );
                    proj.cached_worktrees = vec![];
                }
            }
        }
        info!("app::refresh_worktrees: done in {:.2?}", t.elapsed());
    }

    /// Refresh worktrees for a single project only. Used after worktree
    /// create/remove/merge commands so we don't pay the cost of refreshing all projects.
    pub async fn refresh_worktrees_for_project(
        &mut self,
        project_idx: usize,
        progress_tx: Option<&broadcast::Sender<DaemonMsg>>,
    ) {
        let Some(proj) = self.projects.get_mut(project_idx) else {
            warn!(
                "app::refresh_worktrees_for_project: invalid project_idx={}",
                project_idx
            );
            return;
        };
        info!(
            "app::refresh_worktrees_for_project: fetching for {} (path={})",
            proj.config.name, proj.config.local_path
        );
        let t = std::time::Instant::now();
        if let Some(tx) = progress_tx {
            let _ = tx.send(DaemonMsg::Progress(vec![RefreshStep {
                label: "Worktrees".into(),
                done: 0,
                total: 1,
            }]));
        }
        match worktrunk::fetch_worktrees(&proj.config.local_path).await {
            Ok(wts) => {
                info!(
                    "app::refresh_worktrees_for_project: got {} worktrees for {} in {:.2?}",
                    wts.len(),
                    proj.config.name,
                    t.elapsed()
                );
                if let Some(tx) = progress_tx {
                    let _ = tx.send(DaemonMsg::Progress(vec![RefreshStep {
                        label: "Worktrees".into(),
                        done: 1,
                        total: 1,
                    }]));
                    tokio::task::yield_now().await;
                }
                proj.cached_worktrees = wts;
                if proj.worktree_selected >= proj.cached_worktrees.len()
                    && !proj.cached_worktrees.is_empty()
                {
                    proj.worktree_selected = proj.cached_worktrees.len() - 1;
                }
            }
            Err(e) => {
                warn!(
                    "app::refresh_worktrees_for_project: error for {} after {:.2?}: {}",
                    proj.config.name,
                    t.elapsed(),
                    e
                );
                proj.cached_worktrees = vec![];
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
            default_agent_command: self.default_agent_command.clone(),
            default_worktree_with_prompt: self.default_worktree_with_prompt.clone(),
            keybindings: self.keybindings.clone(),
            pending_changes: Vec::new(),
            agent_actions: self.agent_actions.clone(),
            pending_agent_changes: Vec::new(),
            global_mrs: self.global_mrs.clone(),
            activity_feed: self.activity_feed.iter().cloned().collect(),
        }
    }

    /// Take accumulated pending changes, leaving the internal list empty.
    pub fn take_pending_changes(&mut self) -> Vec<MrChange> {
        std::mem::take(&mut self.pending_changes)
    }

    pub fn take_pending_agent_changes(&mut self) -> Vec<AgentChange> {
        std::mem::take(&mut self.pending_agent_changes)
    }

    fn update_detail(&mut self) {
        self.detail = self.panes.get(self.selected).and_then(|pane| {
            let session_id = pane.db_session_id.as_deref()?;
            let agent = self.find_agent(&pane.pane_command)?;
            agent.fetch_session_detail(session_id)
        });
    }

    pub fn send_agent_prompt(
        &self,
        pane_pid: u32,
        session_id: &str,
        prompt: &str,
    ) -> anyhow::Result<String> {
        let pane = self
            .panes
            .iter()
            .find(|p| p.pane_pid == pane_pid)
            .ok_or_else(|| anyhow::anyhow!("No pane found with PID {}", pane_pid))?;

        let agent = self
            .find_agent(&pane.pane_command)
            .ok_or_else(|| anyhow::anyhow!("No agent registered for '{}'", pane.pane_command))?;

        agent.send_prompt(pane_pid, session_id, prompt)
    }

    fn find_agent(&self, command: &str) -> Option<&dyn CodingAgent> {
        self.agents
            .iter()
            .find(|a| a.process_name() == command)
            .map(|a| a.as_ref())
    }
}

fn detect_mr_list_changes(
    project_name: &str,
    old_mrs: &[MergeRequestSummary],
    new_mrs: &[MergeRequestSummary],
) -> Vec<MrChange> {
    let mut changes = Vec::new();

    for new_mr in new_mrs {
        let Some(old_mr) = old_mrs.iter().find(|m| m.iid == new_mr.iid) else {
            continue;
        };

        if new_mr.user_notes_count > old_mr.user_notes_count {
            let delta = new_mr.user_notes_count - old_mr.user_notes_count;
            changes.push(MrChange {
                project_name: project_name.to_string(),
                mr_iid: new_mr.iid,
                mr_title: new_mr.title.clone(),
                change_type: MrChangeType::NewDiscussions(delta),
            });
        }

        let was_approved = old_mr
            .detailed_merge_status
            .as_deref()
            .is_some_and(|s| s.contains("approved"));
        let is_approved = new_mr
            .detailed_merge_status
            .as_deref()
            .is_some_and(|s| s.contains("approved"));
        if is_approved && !was_approved {
            changes.push(MrChange {
                project_name: project_name.to_string(),
                mr_iid: new_mr.iid,
                mr_title: new_mr.title.clone(),
                change_type: MrChangeType::Approved,
            });
        }
    }

    changes
}

fn detect_mr_detail_changes(
    project_name: &str,
    old_detail: &MergeRequestDetail,
    new_detail: &MergeRequestDetail,
) -> Vec<MrChange> {
    let mut changes = Vec::new();

    if old_detail.iid != new_detail.iid {
        return changes;
    }

    let old_pipeline_status = old_detail.head_pipeline.as_ref().map(|p| p.status.as_str());
    let new_pipeline_status = new_detail.head_pipeline.as_ref().map(|p| p.status.as_str());

    if old_pipeline_status != new_pipeline_status {
        match new_pipeline_status {
            Some("failed") => {
                changes.push(MrChange {
                    project_name: project_name.to_string(),
                    mr_iid: new_detail.iid,
                    mr_title: new_detail.title.clone(),
                    change_type: MrChangeType::PipelineFailed,
                });
            }
            Some("success") => {
                changes.push(MrChange {
                    project_name: project_name.to_string(),
                    mr_iid: new_detail.iid,
                    mr_title: new_detail.title.clone(),
                    change_type: MrChangeType::PipelineSucceeded,
                });
            }
            _ => {}
        }
    }

    changes
}

fn statuses_match(a: &PaneStatus, b: &PaneStatus) -> bool {
    matches!(
        (a, b),
        (PaneStatus::Idle, PaneStatus::Idle)
            | (PaneStatus::Busy, PaneStatus::Busy)
            | (PaneStatus::Unknown, PaneStatus::Unknown)
            | (PaneStatus::Retry { .. }, PaneStatus::Retry { .. })
    )
}

fn agent_change_type(from: &PaneStatus, to: &PaneStatus) -> Option<AgentChangeType> {
    match (from, to) {
        (_, PaneStatus::Busy) => Some(AgentChangeType::Busy),
        (PaneStatus::Busy | PaneStatus::Retry { .. }, PaneStatus::Idle) => {
            Some(AgentChangeType::Idle)
        }
        (_, PaneStatus::Retry { .. }) => Some(AgentChangeType::Retry),
        _ => None,
    }
}
