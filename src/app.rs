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
use crate::protocol::{ActivityEntry, DashboardSnapshot, GlobalMrEntry, ProjectSnapshot};
use crate::read_state::ReadStateDb;
use crate::tmux;
use crate::types::{AgentPane, PaneStatus, SessionDetail};
use crate::worktrunk::{self, WtWorktree};
use jiff::Timestamp as JiffTimestamp;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tracing::{error, warn};

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
        /// Path of the worktree being removed — used to locate the linked tmux window.
        worktree_path: Option<String>,
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
        self.last_refresh = Instant::now();
        self.error = None;

        let process_names: Vec<&str> = self.agents.iter().map(|a| a.process_name()).collect();

        let mut panes = match tmux::list_agent_panes(&process_names) {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(format!("tmux error: {}", e));
                return;
            }
        };

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

        for proj in &mut self.projects {
            match proj.client.fetch_mrs().await {
                Ok(mrs) => {
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
                    last_error = Some(format!("Forge error ({}): {}", proj.config.name, e));
                }
            }
        }

        if let Some(e) = last_error {
            self.error = Some(e);
        }
    }

    pub async fn refresh_global_mrs(&mut self) {
        let mut all_entries: Vec<GlobalMrEntry> = Vec::new();

        // GitLab: aggregate per-project rather than using the global
        // `scope=created_by_me` endpoint.  Project bot tokens (a common setup)
        // only have project-scoped API access, so the global endpoint returns
        // an empty list or only the bot's own MRs.  fetch_mrs() already uses
        // `author_username` and works correctly regardless of token type.
        for proj in &self.projects {
            if !matches!(proj.config.source, ProjectForge::Gitlab) {
                continue;
            }
            match proj.client.fetch_mrs().await {
                Ok(mrs) => {
                    all_entries.extend(mrs.into_iter().map(|mr| GlobalMrEntry {
                        forge: ProjectForge::Gitlab,
                        configured_project: Some(proj.config.name.clone()),
                        mr: crate::forge_clients::types::UserMrSummary {
                            iid: mr.iid,
                            title: mr.title,
                            web_url: mr.web_url.clone(),
                            project_path: proj.config.project.clone(),
                            author: mr.author,
                            draft: mr.draft,
                            updated_at: mr.updated_at,
                        },
                    }));
                }
                Err(e) => error!("global GitLab MR fetch error ({}): {}", proj.config.name, e),
            }
        }

        // GitHub: fetch_user_mrs() uses a user-scoped endpoint that works fine
        // with personal access tokens, so keep the existing approach.
        for proj in &self.projects {
            if !matches!(proj.config.source, ProjectForge::Github) {
                continue;
            }
            match proj.client.fetch_user_mrs().await {
                Ok(mrs) => {
                    all_entries.extend(mrs.into_iter().map(|mr| GlobalMrEntry {
                        forge: ProjectForge::Github,
                        configured_project: Some(proj.config.name.clone()),
                        mr,
                    }));
                }
                Err(e) => error!("global GitHub MR fetch error ({}): {}", proj.config.name, e),
            }
        }

        all_entries.sort_by(|a, b| {
            a.mr.project_path
                .cmp(&b.mr.project_path)
                .then_with(|| b.mr.updated_at.cmp(&a.mr.updated_at))
        });

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

        let project_name = proj.config.name.clone();
        match proj.client.fetch_mr_detail(iid).await {
            Ok(detail) => {
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
                    warn!("worktrunk error ({}): {}", proj.config.name, e);
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
            default_agent_command: self.default_agent_command.clone(),
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
