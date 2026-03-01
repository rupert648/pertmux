use crate::config::{Config, OpencodeConfig};
use crate::types::{OpenCodePane, SessionDetail};
use crate::{api, db, discovery, tmux};
use std::time::{Duration, Instant};

pub struct App {
    pub panes: Vec<OpenCodePane>,
    pub selected: usize,
    pub running: bool,
    pub last_refresh: Instant,
    pub refresh_interval: Duration,
    /// (session_name, vec of indices into self.panes)
    pub groups: Vec<(String, Vec<usize>)>,
    pub error: Option<String>,
    /// Detailed session info for the currently selected pane.
    pub detail: Option<SessionDetail>,
    /// Optional override for the opencode database path.
    pub opencode_config: OpencodeConfig,
}

impl App {
    pub fn new(config: Config) -> Self {
        Self {
            panes: Vec::new(),
            selected: 0,
            running: true,
            last_refresh: Instant::now() - Duration::from_secs(10),
            refresh_interval: Duration::from_secs(config.refresh_interval),
            groups: Vec::new(),
            error: None,
            detail: None,
            opencode_config: config.opencode,
        }
    }

    pub fn refresh(&mut self) {
        self.last_refresh = Instant::now();
        self.error = None;

        let mut panes = match tmux::list_opencode_panes() {
            Ok(p) => p,
            Err(e) => {
                self.error = Some(format!("tmux error: {}", e));
                return;
            }
        };

        for pane in &mut panes {
            // Discover HTTP port
            pane.api_port = discovery::discover_port(pane.pane_pid);

            // Query API for status
            if let Some(port) = pane.api_port {
                let status_map = api::get_session_status(port);
                pane.status = api::status_from_map(&status_map);
            }

            // Enrich from DB
            db::enrich_pane(pane, self.opencode_config.path.as_deref());
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

    fn build_groups(&mut self, panes: &[OpenCodePane]) {
        use std::collections::BTreeMap;
        let mut map: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, pane) in panes.iter().enumerate() {
            map.entry(pane.session_name.clone()).or_default().push(i);
        }
        self.groups = map.into_iter().collect();
    }

    pub fn should_refresh(&self) -> bool {
        self.last_refresh.elapsed() >= self.refresh_interval
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
            .and_then(|id| db::fetch_session_detail(id, self.opencode_config.as_deref()));
    }
}
