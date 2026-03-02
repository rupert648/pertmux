use crate::coding_agent::CodingAgent;
use crate::config::{AgentConfig, Config, GitLabConfig};
use crate::git::discover_worktrees;
use crate::gitlab::client::GitLabClient;
use crate::gitlab::types::{MergeRequestDetail, MergeRequestSummary};
use crate::linking::{link_all, DashboardState};
use crate::read_state::ReadStateDb;
use crate::types::{AgentPane, SessionDetail};
use crate::{coding_agent, db, tmux};
use std::time::{Duration, Instant};

pub enum SelectionSection {
    MergeRequests,
    UnlinkedInstances,
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
    pub dashboard: DashboardState,
    pub cached_mrs: Vec<MergeRequestSummary>,
    pub cached_mr_detail: Option<MergeRequestDetail>,
    pub gitlab_client: Option<GitLabClient>,
    pub read_state: Option<ReadStateDb>,
    pub gitlab_config: Option<GitLabConfig>,
    pub last_detail_refresh: Instant,
    pub detail_refresh_interval: Duration,
    pub selection_section: SelectionSection,
    pub mr_selected: usize,
    pub unlinked_selected: usize,
}

impl App {
    pub fn new(config: Config) -> Self {
        let gitlab_config = config.gitlab.clone();
        let gitlab_client = gitlab_config.as_ref().and_then(|cfg| {
            cfg.api_token()
                .map(|token| GitLabClient::new(token, &cfg.host, &cfg.project))
        });
        let read_state = if gitlab_config.is_some() {
            ReadStateDb::open(None).ok()
        } else {
            None
        };
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
            dashboard: DashboardState {
                linked_mrs: vec![],
                unlinked_instances: vec![],
            },
            cached_mrs: vec![],
            cached_mr_detail: None,
            gitlab_client,
            read_state,
            gitlab_config,
            last_detail_refresh: Instant::now() - Duration::from_secs(120),
            detail_refresh_interval: Duration::from_secs(60),
            selection_section: SelectionSection::MergeRequests,
            mr_selected: 0,
            unlinked_selected: 0,
        }
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
                pane.status = agent.query_status(pane.pane_pid);
            }
            let db_path = self
                .agent_config
                .opencode
                .as_ref()
                .and_then(|c| c.db_path.as_deref());
            db::enrich_pane(pane, db_path);
        }

        // Build groups sorted by session name
        self.build_groups(&panes);
        self.panes = panes;

        // Clamp selection
        if self.selected >= self.panes.len() && !self.panes.is_empty() {
            self.selected = self.panes.len() - 1;
        }

        self.update_detail();

        if let (Some(_), Some(read_state), Some(gitlab_config)) =
            (&self.gitlab_client, &self.read_state, &self.gitlab_config)
        {
            match discover_worktrees(&gitlab_config.local_path).await {
                Ok(worktrees) => {
                    match link_all(
                        &self.cached_mrs,
                        &worktrees,
                        &self.panes,
                        read_state,
                        &gitlab_config.project,
                    ) {
                        Ok(dashboard) => {
                            self.dashboard = dashboard;
                            if self.mr_selected >= self.dashboard.linked_mrs.len()
                                && !self.dashboard.linked_mrs.is_empty()
                            {
                                self.mr_selected = self.dashboard.linked_mrs.len() - 1;
                            }
                            if self.unlinked_selected
                                >= self.dashboard.unlinked_instances.len()
                                && !self.dashboard.unlinked_instances.is_empty()
                            {
                                self.unlinked_selected =
                                    self.dashboard.unlinked_instances.len() - 1;
                            }
                        }
                        Err(e) => {
                            self.error = Some(format!("Linking error: {}", e));
                        }
                    }
                }
                Err(e) => {
                    self.error = Some(format!("Worktree discovery error: {}", e));
                }
            }
        } else {
            self.dashboard = DashboardState {
                linked_mrs: vec![],
                unlinked_instances: vec![],
            };
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
        if self.gitlab_client.is_some() {
            match self.selection_section {
                SelectionSection::MergeRequests => {
                    if self.mr_selected > 0 {
                        self.mr_selected -= 1;
                    }
                }
                SelectionSection::UnlinkedInstances => {
                    if self.unlinked_selected > 0 {
                        self.unlinked_selected -= 1;
                    }
                }
            }
        } else if !self.panes.is_empty() && self.selected > 0 {
            self.selected -= 1;
            self.update_detail();
        }
    }

    pub fn move_down(&mut self) {
        if self.gitlab_client.is_some() {
            match self.selection_section {
                SelectionSection::MergeRequests => {
                    if !self.dashboard.linked_mrs.is_empty()
                        && self.mr_selected < self.dashboard.linked_mrs.len() - 1
                    {
                        self.mr_selected += 1;
                    }
                }
                SelectionSection::UnlinkedInstances => {
                    if !self.dashboard.unlinked_instances.is_empty()
                        && self.unlinked_selected < self.dashboard.unlinked_instances.len() - 1
                    {
                        self.unlinked_selected += 1;
                    }
                }
            }
        } else if !self.panes.is_empty() && self.selected < self.panes.len() - 1 {
            self.selected += 1;
            self.update_detail();
        }
    }

    pub fn toggle_section(&mut self) {
        self.selection_section = match self.selection_section {
            SelectionSection::MergeRequests => SelectionSection::UnlinkedInstances,
            SelectionSection::UnlinkedInstances => SelectionSection::MergeRequests,
        };
    }

    pub fn focus_selected(&self) -> anyhow::Result<()> {
        if self.gitlab_client.is_some() {
            match self.selection_section {
                SelectionSection::MergeRequests => {
                    if let Some(linked) = self.dashboard.linked_mrs.get(self.mr_selected)
                        && let Some(pane) = linked.tmux_pane.as_ref()
                    {
                        tmux::switch_to_pane(&pane.pane_id)?;
                    }
                }
                SelectionSection::UnlinkedInstances => {
                    if let Some(unlinked) = self.dashboard.unlinked_instances.get(self.unlinked_selected)
                    {
                        tmux::switch_to_pane(&unlinked.pane.pane_id)?;
                    }
                }
            }
        } else if let Some(pane) = self.panes.get(self.selected) {
            tmux::switch_to_pane(&pane.pane_id)?;
        }
        Ok(())
    }

    pub async fn refresh_mrs(&mut self) {
        if let (Some(client), Some(_)) = (&self.gitlab_client, &self.gitlab_config) {
            match client.fetch_mr_list().await {
                Ok(mrs) => {
                    self.cached_mrs = mrs;
                    self.error = None;
                }
                Err(e) => {
                    self.error = Some(format!("GitLab error: {}", e));
                }
            }
        }
    }

    pub async fn refresh_mr_detail(&mut self) {
        if let (Some(client), Some(config)) = (&self.gitlab_client, &self.gitlab_config)
            && let Some(linked_mr) = self.dashboard.linked_mrs.get(self.mr_selected)
        {
            let iid = linked_mr.mr.iid;
            match client.fetch_mr_detail(iid).await {
                Ok(detail) => {
                    self.cached_mr_detail = Some(detail);
                    self.last_detail_refresh = Instant::now();
                }
                Err(e) => {
                    self.error = Some(format!("MR detail error: {}", e));
                }
            }

            if let (Ok(notes), Some(rs)) = (client.fetch_mr_notes(iid).await, &self.read_state) {
                let note_ids: Vec<u64> = notes.iter().map(|n| n.id).collect();
                let _ = rs.mark_notes_seen(&config.project, iid, &note_ids);
                let _ = rs.mark_mr_viewed(&config.project, iid, notes.len() as u32);
            }
        }
    }

    pub fn seconds_since_refresh(&self) -> u64 {
        self.last_refresh.elapsed().as_secs()
    }

    /// Fetch session detail for the currently selected pane from the DB.
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

    pub fn open_selected_mr_in_browser(&self) {
        if let Some(linked) = self.dashboard.linked_mrs.get(self.mr_selected) {
            let url = &linked.mr.web_url;
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(url).spawn();
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            let _ = std::process::Command::new("open").arg(url).spawn();
        }
    }

    fn find_agent(&self, command: &str) -> Option<&dyn CodingAgent> {
        self.agents
            .iter()
            .find(|a| a.process_name() == command)
            .map(|a| a.as_ref())
    }
}
