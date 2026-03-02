use crate::coding_agent::CodingAgent;
use crate::config::{AgentConfig, Config};
use crate::types::{AgentPane, SessionDetail};
use crate::{coding_agent, db, tmux};
use std::time::{Duration, Instant};

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
}

impl App {
    pub fn new(config: Config) -> Self {
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
        }
    }

    pub fn refresh(&mut self) {
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
        if !self.panes.is_empty() && self.selected > 0 {
            self.selected -= 1;
            self.update_detail();
        }
    }

    pub fn move_down(&mut self) {
        if !self.panes.is_empty() && self.selected < self.panes.len() - 1 {
            self.selected += 1;
            self.update_detail();
        }
    }

    pub fn focus_selected(&self) -> anyhow::Result<()> {
        if let Some(pane) = self.panes.get(self.selected) {
            tmux::switch_to_pane(&pane.pane_id)?;
        }
        Ok(())
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

    fn find_agent(&self, command: &str) -> Option<&dyn CodingAgent> {
        self.agents
            .iter()
            .find(|a| a.process_name() == command)
            .map(|a| a.as_ref())
    }
}
