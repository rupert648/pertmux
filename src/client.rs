use crate::app::{PopupState, SelectionSection};
use crate::banner::{DIM, GRAY, GREEN, ORANGE, RESET, WHITE};
use crate::daemon;
use crate::protocol::{ClientMsg, DaemonMsg, DashboardSnapshot, PROTOCOL_VERSION};
use crate::tmux;
use crate::ui;
use anyhow::Result;
use bytes::Bytes;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::{SinkExt, StreamExt};
use ratatui::prelude::*;
use std::io;
use std::path::PathBuf;
use std::time::Instant;
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

fn last_project_path() -> Option<PathBuf> {
    let data_dir = dirs::data_dir()?;
    Some(data_dir.join("pertmux").join("last_project"))
}

fn save_last_project(name: &str) {
    if let Some(path) = last_project_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, name);
    }
}

fn load_last_project() -> Option<String> {
    let path = last_project_path()?;
    std::fs::read_to_string(&path)
        .ok()
        .map(|s| s.trim().to_string())
}

fn open_url_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let _ = std::process::Command::new("open").arg(url).spawn();
}

pub struct ClientState {
    pub snapshot: DashboardSnapshot,
    pub active_project: usize,
    pub mr_selected: Vec<usize>,
    pub worktree_selected: Vec<usize>,
    pub selection_section: Vec<SelectionSection>,
    pub selected: usize,
    pub popup: PopupState,
    pub notification: Option<(String, Instant)>,
    pub running: bool,
}

impl ClientState {
    fn from_snapshot(mut snapshot: DashboardSnapshot) -> Self {
        let n = snapshot.projects.len();
        let active_project = load_last_project()
            .and_then(|name| snapshot.projects.iter().position(|p| p.name == name))
            .unwrap_or(0);
        let popup = if !snapshot.pending_changes.is_empty() {
            let changes = std::mem::take(&mut snapshot.pending_changes);
            PopupState::ChangeSummary {
                changes,
                selected: 0,
            }
        } else {
            PopupState::None
        };
        Self {
            snapshot,
            active_project,
            mr_selected: vec![0; n],
            worktree_selected: vec![0; n],
            selection_section: (0..n).map(|_| SelectionSection::Worktrees).collect(),
            selected: 0,
            popup,
            notification: None,
            running: true,
        }
    }

    fn update_snapshot(&mut self, mut snapshot: DashboardSnapshot) {
        // Show a toast notification for any MR changes that arrived while connected.
        // (The activity feed itself is managed entirely by the daemon and arrives
        // pre-populated in snapshot.activity_feed — no client-side conversion needed.)
        if !snapshot.pending_changes.is_empty() {
            let changes = std::mem::take(&mut snapshot.pending_changes);
            let summary: String = changes
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            self.notification = Some((summary, Instant::now()));
        }

        while self.mr_selected.len() < snapshot.projects.len() {
            self.mr_selected.push(0);
            self.worktree_selected.push(0);
            self.selection_section.push(SelectionSection::Worktrees);
        }

        for (i, proj) in snapshot.projects.iter().enumerate() {
            if self.mr_selected[i] >= proj.dashboard.linked_mrs.len()
                && !proj.dashboard.linked_mrs.is_empty()
            {
                self.mr_selected[i] = proj.dashboard.linked_mrs.len() - 1;
            }
            if self.worktree_selected[i] >= proj.cached_worktrees.len()
                && !proj.cached_worktrees.is_empty()
            {
                self.worktree_selected[i] = proj.cached_worktrees.len() - 1;
            }
        }

        if self.active_project >= snapshot.projects.len() && !snapshot.projects.is_empty() {
            self.active_project = snapshot.projects.len() - 1;
        }
        if self.selected >= snapshot.panes.len() && !snapshot.panes.is_empty() {
            self.selected = snapshot.panes.len() - 1;
        }

        self.snapshot = snapshot;
    }

    fn has_projects(&self) -> bool {
        !self.snapshot.projects.is_empty()
    }

    fn active_project(&self) -> Option<&crate::protocol::ProjectSnapshot> {
        self.snapshot.projects.get(self.active_project)
    }

    fn has_popup(&self) -> bool {
        !matches!(self.popup, PopupState::None)
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.notification = Some((msg.into(), Instant::now()));
    }

    fn move_up(&mut self) {
        if let Some(proj) = self.snapshot.projects.get(self.active_project) {
            match self
                .selection_section
                .get(self.active_project)
                .unwrap_or(&SelectionSection::Worktrees)
            {
                SelectionSection::MergeRequests => {
                    if self.mr_selected[self.active_project] > 0 {
                        self.mr_selected[self.active_project] -= 1;
                    }
                }
                SelectionSection::Worktrees => {
                    if self.worktree_selected[self.active_project] > 0 {
                        self.worktree_selected[self.active_project] -= 1;
                    }
                }
            }

            if self.mr_selected[self.active_project] >= proj.dashboard.linked_mrs.len()
                && !proj.dashboard.linked_mrs.is_empty()
            {
                self.mr_selected[self.active_project] = proj.dashboard.linked_mrs.len() - 1;
            }
        } else if !self.snapshot.panes.is_empty() && self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn move_down(&mut self) {
        if let Some(proj) = self.snapshot.projects.get(self.active_project) {
            match self
                .selection_section
                .get(self.active_project)
                .unwrap_or(&SelectionSection::Worktrees)
            {
                SelectionSection::MergeRequests => {
                    if !proj.dashboard.linked_mrs.is_empty()
                        && self.mr_selected[self.active_project]
                            < proj.dashboard.linked_mrs.len() - 1
                    {
                        self.mr_selected[self.active_project] += 1;
                    }
                }
                SelectionSection::Worktrees => {
                    if !proj.cached_worktrees.is_empty()
                        && self.worktree_selected[self.active_project]
                            < proj.cached_worktrees.len() - 1
                    {
                        self.worktree_selected[self.active_project] += 1;
                    }
                }
            }
        } else if !self.snapshot.panes.is_empty() && self.selected < self.snapshot.panes.len() - 1 {
            self.selected += 1;
        }
    }

    fn toggle_section(&mut self) {
        if self.snapshot.projects.get(self.active_project).is_some() {
            let section = self
                .selection_section
                .get_mut(self.active_project)
                .expect("selection section exists for project");
            *section = match section {
                SelectionSection::MergeRequests => SelectionSection::Worktrees,
                SelectionSection::Worktrees => SelectionSection::MergeRequests,
            };
        }
    }

    fn current_mr_iid(&self) -> Option<u64> {
        let proj = self.active_project()?;
        if !matches!(
            self.selection_section.get(self.active_project),
            Some(SelectionSection::MergeRequests)
        ) {
            return None;
        }
        proj.dashboard
            .linked_mrs
            .get(*self.mr_selected.get(self.active_project).unwrap_or(&0))
            .map(|l| l.mr.iid)
    }

    fn open_selected_mr_in_browser(&self) {
        if let Some(proj) = self.snapshot.projects.get(self.active_project)
            && let Some(linked) = proj
                .dashboard
                .linked_mrs
                .get(*self.mr_selected.get(self.active_project).unwrap_or(&0))
        {
            open_url_in_browser(&linked.mr.web_url);
        }
    }

    fn open_mr_overview(&mut self) {
        if self.snapshot.global_mrs.is_empty() {
            self.notify("No open MRs found");
            return;
        }
        self.popup = PopupState::MrOverview { selected: 0 };
    }

    fn open_activity_feed(&mut self) {
        if self.snapshot.activity_feed.is_empty() {
            self.notify("No activity yet");
            return;
        }
        self.popup = PopupState::ActivityFeed { selected: 0 };
    }

    fn copy_selected_branch(&mut self) {
        let branch = if let Some(proj) = self.snapshot.projects.get(self.active_project) {
            match self
                .selection_section
                .get(self.active_project)
                .unwrap_or(&SelectionSection::Worktrees)
            {
                SelectionSection::MergeRequests => proj
                    .dashboard
                    .linked_mrs
                    .get(*self.mr_selected.get(self.active_project).unwrap_or(&0))
                    .map(|l| l.mr.source_branch.clone()),
                SelectionSection::Worktrees => proj
                    .cached_worktrees
                    .get(
                        *self
                            .worktree_selected
                            .get(self.active_project)
                            .unwrap_or(&0),
                    )
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
                self.notify(format!("Copied: {}", branch));
            }
        }
    }

    fn open_create_popup(&mut self) {
        if let Some(_proj) = self.snapshot.projects.get(self.active_project)
            && matches!(
                self.selection_section.get(self.active_project),
                Some(SelectionSection::Worktrees)
            )
        {
            self.popup = PopupState::CreateWorktree {
                input: String::new(),
            };
        }
    }

    fn open_remove_popup(&mut self) {
        if let Some(proj) = self.snapshot.projects.get(self.active_project)
            && matches!(
                self.selection_section.get(self.active_project),
                Some(SelectionSection::Worktrees)
            )
            && let Some(wt) = proj.cached_worktrees.get(
                *self
                    .worktree_selected
                    .get(self.active_project)
                    .unwrap_or(&0),
            )
        {
            if wt.is_main {
                self.notify("Cannot remove main worktree");
                return;
            }
            if let Some(ref branch) = wt.branch {
                self.popup = PopupState::ConfirmRemove {
                    branch: branch.clone(),
                    worktree_path: wt.path.clone(),
                };
            }
        }
    }

    fn open_merge_popup(&mut self) {
        if let Some(proj) = self.snapshot.projects.get(self.active_project)
            && matches!(
                self.selection_section.get(self.active_project),
                Some(SelectionSection::Worktrees)
            )
            && let Some(wt) = proj.cached_worktrees.get(
                *self
                    .worktree_selected
                    .get(self.active_project)
                    .unwrap_or(&0),
            )
        {
            if wt.is_main {
                self.notify("Cannot merge main worktree");
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

    fn popup_input_push(&mut self, ch: char) {
        if let PopupState::CreateWorktree { ref mut input } = self.popup {
            input.push(ch);
        }
    }

    fn popup_input_pop(&mut self) {
        if let PopupState::CreateWorktree { ref mut input } = self.popup {
            input.pop();
        }
    }

    fn close_popup(&mut self) {
        self.popup = PopupState::None;
    }

    fn open_agent_actions(&mut self) {
        let proj = match self.snapshot.projects.get(self.active_project) {
            Some(p) => p,
            None => {
                self.notify("No active project");
                return;
            }
        };

        let wt_idx = *self
            .worktree_selected
            .get(self.active_project)
            .unwrap_or(&0);
        let wt = match proj.cached_worktrees.get(wt_idx) {
            Some(wt) => wt,
            None => {
                self.notify("No worktree selected");
                return;
            }
        };

        let wt_path = match &wt.path {
            Some(p) => p.clone(),
            None => {
                self.notify("Worktree has no path");
                return;
            }
        };

        let canonical_wt = std::fs::canonicalize(&wt_path).ok();
        let pane = self.snapshot.panes.iter().find(|p| {
            canonical_wt
                .as_ref()
                .and_then(|cwt| {
                    std::fs::canonicalize(&p.pane_path)
                        .ok()
                        .map(|cp| cp == *cwt)
                })
                .unwrap_or(false)
        });

        let pane = match pane {
            Some(p) => p,
            None => {
                self.notify("No agent instance for this worktree");
                return;
            }
        };

        let session_id = match &pane.db_session_id {
            Some(id) => id.clone(),
            None => {
                self.notify("No active agent session");
                return;
            }
        };

        self.popup = PopupState::AgentActions {
            selected: 0,
            pane_pid: pane.pane_pid,
            session_id,
            worktree_branch: wt.branch.clone(),
        };
    }

    fn open_project_filter(&mut self) {
        if self.snapshot.projects.len() < 2 {
            return;
        }
        let all: Vec<(usize, String)> = self
            .snapshot
            .projects
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.name.clone()))
            .collect();
        self.popup = PopupState::ProjectFilter {
            input: String::new(),
            filtered: all,
            selected: 0,
        };
    }

    fn recompute_project_filter(&mut self) {
        if let PopupState::ProjectFilter {
            input,
            filtered,
            selected,
        } = &mut self.popup
        {
            let projects: Vec<(usize, &str)> = self
                .snapshot
                .projects
                .iter()
                .enumerate()
                .map(|(i, p)| (i, p.name.as_str()))
                .collect();

            if input.is_empty() {
                *filtered = projects.iter().map(|(i, n)| (*i, n.to_string())).collect();
            } else {
                use nucleo_matcher::Matcher;
                use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};

                let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
                let pattern = Pattern::parse(input, CaseMatching::Ignore, Normalization::Smart);
                let names: Vec<&str> = projects.iter().map(|(_, n)| *n).collect();
                let matches = pattern.match_list(names, &mut matcher);

                *filtered = matches
                    .into_iter()
                    .filter_map(|(name, _score)| {
                        projects
                            .iter()
                            .find(|(_, n)| *n == name)
                            .map(|(i, _)| (*i, name.to_string()))
                    })
                    .collect();
            }

            if *selected >= filtered.len() {
                *selected = filtered.len().saturating_sub(1);
            }
        }
    }
}

pub async fn run() -> Result<()> {
    let sock_path = daemon::socket_path();
    let stream = match UnixStream::connect(&sock_path).await {
        Ok(s) => s,
        Err(_) => {
            show_connection_error(&sock_path);
            return Ok(());
        }
    };
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    let handshake = ClientMsg::Handshake {
        version: PROTOCOL_VERSION,
    };
    framed
        .send(Bytes::from(serde_json::to_vec(&handshake)?))
        .await?;

    let initial_snapshot = wait_for_initial_snapshot(&mut framed).await?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = ClientState::from_snapshot(initial_snapshot);
    let result = run_client_loop(&mut terminal, &mut state, &mut framed).await;

    if let Some(proj) = state.snapshot.projects.get(state.active_project) {
        save_last_project(&proj.name);
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

pub async fn stop() -> Result<()> {
    let sock_path = daemon::socket_path();
    let stream = UnixStream::connect(&sock_path)
        .await
        .map_err(|_| anyhow::anyhow!("no daemon running at {}", sock_path.display()))?;
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    // Drain the initial snapshot the daemon sends on connect,
    // otherwise our Stop message is never read.
    let _ = framed.next().await;

    let msg = ClientMsg::Stop;
    framed.send(Bytes::from(serde_json::to_vec(&msg)?)).await?;

    crate::banner::print();
    println!("  {GRAY}daemon stopped{RESET}");
    println!();
    Ok(())
}

/// Returns the path to the most recent pertmux daemon log file, if any.
fn latest_log_path() -> Option<std::path::PathBuf> {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir("/tmp")
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            s.starts_with("pertmux-daemon-") && s.ends_with(".log")
        })
        .map(|e| e.path())
        .collect();
    entries.sort();
    entries.pop()
}

pub fn status() {
    let sock_path = daemon::socket_path();
    let log = latest_log_path()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/tmp/pertmux-daemon-*.log".to_string());

    crate::banner::print();

    if !sock_path.exists() {
        println!("  {GRAY}daemon{RESET}  {GRAY}○{RESET}  not running  {DIM}(no socket){RESET}");
        println!("  {GRAY}socket{RESET}  {DIM}{}{RESET}", sock_path.display());
        println!("  {GRAY}log   {RESET}  {DIM}{}{RESET}", log);
        println!();
        println!("  {DIM}start with{RESET}  {ORANGE}pertmux serve{RESET}");
    } else {
        let probe = std::os::unix::net::UnixStream::connect(&sock_path);
        match probe {
            Ok(_) => {
                println!("  {GRAY}daemon{RESET}  {GREEN}●{RESET}  {WHITE}running{RESET}");
            }
            Err(_) => {
                println!(
                    "  {GRAY}daemon{RESET}  {GRAY}◐{RESET}  stale socket  {DIM}(not responding){RESET}"
                );
                println!();
                println!("  {DIM}clean up with{RESET}  {ORANGE}pertmux cleanup{RESET}");
            }
        }
        println!("  {GRAY}socket{RESET}  {}", sock_path.display());
        println!("  {GRAY}log   {RESET}  {DIM}{}{RESET}", log);
    }
    println!();
}

pub fn cleanup() -> anyhow::Result<()> {
    crate::banner::print();

    let sock_path = daemon::socket_path();
    if sock_path.exists() {
        let is_stale = std::os::unix::net::UnixStream::connect(&sock_path).is_err();
        if is_stale {
            std::fs::remove_file(&sock_path)?;
            println!(
                "  {GREEN}✓{RESET}  stale socket removed  {DIM}{}{RESET}",
                sock_path.display()
            );
        } else {
            println!("  {GRAY}─{RESET}  socket is live (daemon running), skipping");
        }
    } else {
        println!("  {GRAY}─{RESET}  no socket found");
    }

    if let Some(data_dir) = dirs::data_dir() {
        let pertmux_dir = data_dir.join("pertmux");

        let read_state_path = pertmux_dir.join("read_state.db");
        if read_state_path.exists() {
            std::fs::remove_file(&read_state_path)?;
            println!(
                "  {GREEN}✓{RESET}  read state removed    {DIM}{}{RESET}",
                read_state_path.display()
            );
        }

        let last_project_path = pertmux_dir.join("last_project");
        if last_project_path.exists() {
            std::fs::remove_file(&last_project_path)?;
            println!(
                "  {GREEN}✓{RESET}  last project removed  {DIM}{}{RESET}",
                last_project_path.display()
            );
        }
    }

    println!();
    println!("  {GRAY}done{RESET}");
    println!();
    Ok(())
}

async fn wait_for_initial_snapshot(
    framed: &mut Framed<UnixStream, LengthDelimitedCodec>,
) -> Result<DashboardSnapshot> {
    while let Some(frame) = framed.next().await {
        let bytes = frame?;
        let msg: DaemonMsg = serde_json::from_slice(&bytes)?;
        match msg {
            DaemonMsg::Snapshot(snap) => return Ok(*snap),
            DaemonMsg::HandshakeAck { .. } => {}
            DaemonMsg::ActionResult { ok, message } => {
                if !ok {
                    anyhow::bail!(message);
                }
            }
        }
    }
    anyhow::bail!("daemon disconnected before initial snapshot")
}

async fn run_client_loop(
    terminal: &mut Terminal<impl Backend>,
    state: &mut ClientState,
    framed: &mut Framed<UnixStream, LengthDelimitedCodec>,
) -> Result<()> {
    let mut event_stream = EventStream::new();

    while state.running {
        terminal.draw(|frame| ui::draw_client(frame, state))?;

        tokio::select! {
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        handle_key(state, framed, key.code).await?;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => {}
                    None => break,
                }
            }
            msg = framed.next() => {
                match msg {
                    Some(Ok(bytes)) => {
                        let daemon_msg: DaemonMsg = serde_json::from_slice(&bytes)?;
                        match daemon_msg {
                            DaemonMsg::Snapshot(snap) => {
                                state.update_snapshot(*snap);
                            }
                            DaemonMsg::ActionResult { ok, message } => {
                                state.notify(message);
                                if ok {
                                    // After a successful worktree removal, offer to kill the
                                    // linked tmux window if one is associated with that path.
                                    let kill_window_data =
                                        if let PopupState::ConfirmRemove { branch, worktree_path } =
                                            &state.popup
                                        {
                                            let maybe_pane_id =
                                                worktree_path.as_ref().and_then(|path| {
                                                    let canonical =
                                                        std::fs::canonicalize(path).ok();
                                                    state.snapshot.panes.iter().find(|p| {
                                                        canonical
                                                            .as_ref()
                                                            .and_then(|cp| {
                                                                std::fs::canonicalize(&p.pane_path)
                                                                    .ok()
                                                                    .map(|pp| pp == *cp)
                                                            })
                                                            .unwrap_or(false)
                                                    })
                                                }).map(|p| p.pane_id.clone());
                                            maybe_pane_id
                                                .map(|pane_id| (branch.clone(), pane_id))
                                        } else {
                                            None
                                        };

                                    if let Some((branch, pane_id)) = kill_window_data {
                                        state.popup =
                                            PopupState::ConfirmKillTmuxWindow { branch, pane_id };
                                    } else {
                                        state.popup = PopupState::None;
                                    }
                                }
                            }
                            DaemonMsg::HandshakeAck { .. } => {}
                        }
                    }
                    Some(Err(_)) => break,
                    None => {
                        anyhow::bail!("daemon disconnected");
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_key(
    state: &mut ClientState,
    framed: &mut Framed<UnixStream, LengthDelimitedCodec>,
    code: KeyCode,
) -> Result<()> {
    if matches!(state.popup, PopupState::ProjectFilter { .. }) {
        match code {
            KeyCode::Esc => state.close_popup(),
            KeyCode::Enter => {
                if let PopupState::ProjectFilter {
                    filtered, selected, ..
                } = &state.popup
                    && let Some(&(idx, _)) = filtered.get(*selected)
                {
                    state.active_project = idx;
                    if let Some(proj) = state.snapshot.projects.get(idx) {
                        save_last_project(&proj.name);
                    }
                }
                state.close_popup();
                if let Some(mr_iid) = state.current_mr_iid() {
                    send_msg(
                        framed,
                        ClientMsg::SelectMr {
                            project_idx: state.active_project,
                            mr_iid,
                        },
                    )
                    .await?;
                }
            }
            KeyCode::Down => {
                if let PopupState::ProjectFilter {
                    filtered, selected, ..
                } = &mut state.popup
                    && *selected + 1 < filtered.len()
                {
                    *selected += 1;
                }
            }
            KeyCode::Up => {
                if let PopupState::ProjectFilter { selected, .. } = &mut state.popup
                    && *selected > 0
                {
                    *selected -= 1;
                }
            }
            KeyCode::Backspace => {
                if let PopupState::ProjectFilter { input, .. } = &mut state.popup {
                    input.pop();
                }
                state.recompute_project_filter();
            }
            KeyCode::Char(ch) => {
                if let PopupState::ProjectFilter { input, .. } = &mut state.popup {
                    input.push(ch);
                }
                state.recompute_project_filter();
            }
            _ => {}
        }
        return Ok(());
    }

    if matches!(state.popup, PopupState::ChangeSummary { .. }) {
        match code {
            KeyCode::Esc => state.close_popup(),
            KeyCode::Up | KeyCode::Char('k') => {
                if let PopupState::ChangeSummary { selected, .. } = &mut state.popup
                    && *selected > 0
                {
                    *selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let PopupState::ChangeSummary {
                    changes, selected, ..
                } = &mut state.popup
                    && *selected + 1 < changes.len()
                {
                    *selected += 1;
                }
            }
            KeyCode::Enter => {
                if let PopupState::ChangeSummary { changes, selected } =
                    std::mem::replace(&mut state.popup, PopupState::None)
                    && let Some(change) = changes.get(selected)
                    && let Some(idx) = state
                        .snapshot
                        .projects
                        .iter()
                        .position(|p| p.name == change.project_name)
                {
                    state.active_project = idx;
                    if let Some(proj) = state.snapshot.projects.get(idx) {
                        save_last_project(&proj.name);
                        if let Some(mr_idx) = proj
                            .dashboard
                            .linked_mrs
                            .iter()
                            .position(|l| l.mr.iid == change.mr_iid)
                        {
                            state.mr_selected[idx] = mr_idx;
                            state.selection_section[idx] = SelectionSection::MergeRequests;
                            send_msg(
                                framed,
                                ClientMsg::SelectMr {
                                    project_idx: idx,
                                    mr_iid: change.mr_iid,
                                },
                            )
                            .await?;
                        }
                    }
                }
            }
            _ => {}
        }
        return Ok(());
    }

    if matches!(state.popup, PopupState::MrOverview { .. }) {
        match code {
            KeyCode::Esc => state.close_popup(),
            KeyCode::Up | KeyCode::Char('k') => {
                if let PopupState::MrOverview { selected } = &mut state.popup
                    && *selected > 0
                {
                    *selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let PopupState::MrOverview { selected } = &mut state.popup
                    && *selected + 1 < state.snapshot.global_mrs.len()
                {
                    *selected += 1;
                }
            }
            KeyCode::Enter => {
                if let PopupState::MrOverview { selected } =
                    std::mem::replace(&mut state.popup, PopupState::None)
                    && let Some(entry) = state.snapshot.global_mrs.get(selected)
                {
                    if let Some(ref proj_name) = entry.configured_project {
                        // Navigate to the configured project
                        if let Some(idx) = state
                            .snapshot
                            .projects
                            .iter()
                            .position(|p| &p.name == proj_name)
                        {
                            state.active_project = idx;
                            save_last_project(proj_name);
                            // Select MR section
                            if let Some(section) = state.selection_section.get_mut(idx) {
                                *section = SelectionSection::MergeRequests;
                            }
                            // Try to find the MR in linked_mrs and select it
                            let iid = entry.mr.iid;
                            if let Some(proj) = state.snapshot.projects.get(idx)
                                && let Some(mr_idx) = proj
                                    .dashboard
                                    .linked_mrs
                                    .iter()
                                    .position(|l| l.mr.iid == iid)
                            {
                                state.mr_selected[idx] = mr_idx;
                                send_msg(
                                    framed,
                                    ClientMsg::SelectMr {
                                        project_idx: idx,
                                        mr_iid: iid,
                                    },
                                )
                                .await?;
                            }
                        }
                    } else {
                        // Not a configured project — open in browser
                        open_url_in_browser(&entry.mr.web_url);
                        state.notify("Opened in browser (project not configured)");
                    }
                }
            }
            _ => {}
        }
        return Ok(());
    }

    if matches!(state.popup, PopupState::ActivityFeed { .. }) {
        match code {
            KeyCode::Esc => state.close_popup(),
            KeyCode::Up | KeyCode::Char('k') => {
                if let PopupState::ActivityFeed { selected } = &mut state.popup
                    && *selected > 0
                {
                    *selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let PopupState::ActivityFeed { selected } = &mut state.popup
                    && *selected + 1 < state.snapshot.activity_feed.len()
                {
                    *selected += 1;
                }
            }
            KeyCode::Enter => {
                if let PopupState::ActivityFeed { selected } =
                    std::mem::replace(&mut state.popup, PopupState::None)
                    && let Some(entry) = state.snapshot.activity_feed.get(selected).cloned()
                {
                    navigate_to_activity(state, framed, &entry).await?;
                }
            }
            _ => {}
        }
        return Ok(());
    }

    if matches!(state.popup, PopupState::AgentActions { .. }) {
        match code {
            KeyCode::Esc => state.close_popup(),
            KeyCode::Up | KeyCode::Char('k') => {
                if let PopupState::AgentActions { selected, .. } = &mut state.popup
                    && *selected > 0
                {
                    *selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let PopupState::AgentActions { selected, .. } = &mut state.popup
                    && *selected + 1 < state.snapshot.agent_actions.len()
                {
                    *selected += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(msg) = build_agent_action_msg(state) {
                    state.notify("Sending to agent...");
                    send_msg(framed, msg).await?;
                }
                state.close_popup();
            }
            _ => {}
        }
        return Ok(());
    }

    if matches!(state.popup, PopupState::ConfirmKillTmuxWindow { .. }) {
        match code {
            KeyCode::Esc => state.close_popup(),
            KeyCode::Enter => {
                let pane_id =
                    if let PopupState::ConfirmKillTmuxWindow { ref pane_id, .. } = state.popup {
                        Some(pane_id.clone())
                    } else {
                        None
                    };
                if let Some(pane_id) = pane_id {
                    if let Err(e) = tmux::kill_window(&pane_id) {
                        state.notify(format!("Kill window failed: {}", e));
                    } else {
                        state.notify("Tmux window closed");
                    }
                }
                state.close_popup();
            }
            _ => {}
        }
        return Ok(());
    }

    if state.has_popup() {
        match code {
            KeyCode::Esc => state.close_popup(),
            KeyCode::Enter => {
                if let Some(msg) = popup_action_msg(state) {
                    let toast = match &state.popup {
                        PopupState::CreateWorktree { .. } => "Creating worktree...",
                        PopupState::ConfirmRemove { .. } => "Removing worktree...",
                        PopupState::ConfirmMerge { .. } => "Merging worktree...",
                        _ => "",
                    };
                    if !toast.is_empty() {
                        state.notify(toast);
                    }
                    send_msg(framed, msg).await?;
                }
            }
            KeyCode::Backspace => state.popup_input_pop(),
            KeyCode::Char(ch) => {
                if matches!(state.popup, PopupState::CreateWorktree { .. }) {
                    state.popup_input_push(ch);
                }
            }
            _ => {}
        }
        return Ok(());
    }

    match code {
        KeyCode::Char('q') | KeyCode::Esc => state.running = false,
        KeyCode::Up | KeyCode::Char('k') => {
            let before = state.current_mr_iid();
            state.move_up();
            maybe_send_select_mr(state, framed, before).await?;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let before = state.current_mr_iid();
            state.move_down();
            maybe_send_select_mr(state, framed, before).await?;
        }
        KeyCode::Tab => {
            let before = state.current_mr_iid();
            state.toggle_section();
            maybe_send_select_mr(state, framed, before).await?;
        }
        KeyCode::Enter => {
            if let Err(e) = focus_selected(state) {
                state.notify(format!("Focus failed: {}", e));
            }
        }
        KeyCode::Char(ch) => {
            let kb = &state.snapshot.keybindings;
            if ch == kb.refresh {
                state.notify("Refreshing...");
                send_msg(framed, ClientMsg::Refresh).await?;
            } else if ch == kb.open_browser {
                if state.has_projects() {
                    state.open_selected_mr_in_browser();
                }
            } else if ch == kb.copy_branch {
                if state.has_projects() {
                    state.copy_selected_branch();
                }
            } else if ch == kb.filter_projects {
                state.open_project_filter();
            } else if ch == kb.create_worktree {
                state.open_create_popup();
            } else if ch == kb.delete_worktree {
                state.open_remove_popup();
            } else if ch == kb.merge_worktree {
                state.open_merge_popup();
            } else if ch == kb.agent_actions {
                state.open_agent_actions();
            } else if ch == kb.mr_overview {
                state.open_mr_overview();
            } else if ch == kb.activity_feed {
                state.open_activity_feed();
            }
        }
        _ => {}
    }

    Ok(())
}

fn popup_action_msg(state: &ClientState) -> Option<ClientMsg> {
    let project_idx = state.active_project;
    match &state.popup {
        PopupState::CreateWorktree { input } => {
            let branch = input.trim().to_string();
            if branch.is_empty() {
                return None;
            }
            Some(ClientMsg::CreateWorktree {
                project_idx,
                branch,
            })
        }
        PopupState::ConfirmRemove { branch, .. } => Some(ClientMsg::RemoveWorktree {
            project_idx,
            branch: branch.clone(),
        }),
        PopupState::ConfirmMerge { worktree_path, .. } => Some(ClientMsg::MergeWorktree {
            project_idx,
            worktree_path: worktree_path.clone(),
        }),
        PopupState::ProjectFilter { .. }
        | PopupState::ChangeSummary { .. }
        | PopupState::AgentActions { .. }
        | PopupState::MrOverview { .. }
        | PopupState::ActivityFeed { .. }
        | PopupState::ConfirmKillTmuxWindow { .. }
        | PopupState::None => None,
    }
}

fn build_agent_action_msg(state: &ClientState) -> Option<ClientMsg> {
    let PopupState::AgentActions {
        selected,
        pane_pid,
        session_id,
        worktree_branch,
    } = &state.popup
    else {
        return None;
    };

    let action = state.snapshot.agent_actions.get(*selected)?;
    let proj = state.snapshot.projects.get(state.active_project)?;

    let linked_mr = worktree_branch.as_ref().and_then(|branch| {
        proj.dashboard
            .linked_mrs
            .iter()
            .find(|l| &l.mr.source_branch == branch)
    });

    // If the action requires an MR but none is linked, bail out
    if action.requires_mr && linked_mr.is_none() {
        return None;
    }

    let prompt = substitute_template(&action.prompt, linked_mr.map(|l| &l.mr), &proj.name);

    Some(ClientMsg::AgentAction {
        pane_pid: *pane_pid,
        session_id: session_id.clone(),
        prompt,
    })
}

fn substitute_template(
    template: &str,
    mr: Option<&crate::forge_clients::types::MergeRequestSummary>,
    project_name: &str,
) -> String {
    let mut result = template.to_string();
    result = result.replace("{project_name}", project_name);

    if let Some(mr) = mr {
        result = result.replace("{target_branch}", &mr.target_branch);
        result = result.replace("{source_branch}", &mr.source_branch);
        result = result.replace("{mr_url}", &mr.web_url);
        result = result.replace("{mr_iid}", &mr.iid.to_string());
    } else {
        // Provide sensible defaults when no MR is linked
        result = result.replace("{target_branch}", "main");
        result = result.replace("{source_branch}", "");
        result = result.replace("{mr_url}", "");
        result = result.replace("{mr_iid}", "");
    }

    result
}

async fn maybe_send_select_mr(
    state: &ClientState,
    framed: &mut Framed<UnixStream, LengthDelimitedCodec>,
    before: Option<u64>,
) -> Result<()> {
    let after = state.current_mr_iid();
    if after != before
        && let Some(mr_iid) = after
    {
        send_msg(
            framed,
            ClientMsg::SelectMr {
                project_idx: state.active_project,
                mr_iid,
            },
        )
        .await?;
    }
    Ok(())
}

fn focus_selected(state: &ClientState) -> Result<()> {
    if let Some(proj) = state.snapshot.projects.get(state.active_project) {
        match state
            .selection_section
            .get(state.active_project)
            .unwrap_or(&SelectionSection::Worktrees)
        {
            SelectionSection::MergeRequests => {
                if let Some(linked) = proj
                    .dashboard
                    .linked_mrs
                    .get(*state.mr_selected.get(state.active_project).unwrap_or(&0))
                    && let Some(pane) = linked.tmux_pane.as_ref()
                {
                    tmux::switch_to_pane(&pane.pane_id)?;
                }
            }
            SelectionSection::Worktrees => {
                if let Some(wt) = proj.cached_worktrees.get(
                    *state
                        .worktree_selected
                        .get(state.active_project)
                        .unwrap_or(&0),
                ) && let Some(ref path) = wt.path
                {
                    tmux::find_or_create_pane(
                        path,
                        &proj.name,
                        state.snapshot.default_agent_command.as_deref(),
                    )?;
                }
            }
        }
    } else if let Some(pane) = state.snapshot.panes.get(state.selected) {
        tmux::switch_to_pane(&pane.pane_id)?;
    }
    Ok(())
}

/// Navigate pertmux to the item referenced by an activity entry.
///
/// Agent activities switch the tmux client to the recorded pane.
/// MR activities select the matching project + MR in the main view.
async fn navigate_to_activity(
    state: &mut ClientState,
    framed: &mut Framed<UnixStream, LengthDelimitedCodec>,
    entry: &crate::protocol::ActivityEntry,
) -> Result<()> {
    use crate::protocol::ActivityTarget;
    match &entry.target {
        Some(ActivityTarget::Pane { pane_id, .. }) => {
            if let Err(e) = tmux::switch_to_pane(pane_id) {
                state.notify(format!("Pane no longer active: {}", e));
            }
        }
        Some(ActivityTarget::MergeRequest { project_name, iid }) => {
            if let Some(idx) = state
                .snapshot
                .projects
                .iter()
                .position(|p| &p.name == project_name)
            {
                state.active_project = idx;
                save_last_project(project_name);
                if let Some(section) = state.selection_section.get_mut(idx) {
                    *section = SelectionSection::MergeRequests;
                }
                let iid = *iid;
                if let Some(proj) = state.snapshot.projects.get(idx)
                    && let Some(mr_idx) = proj
                        .dashboard
                        .linked_mrs
                        .iter()
                        .position(|l| l.mr.iid == iid)
                {
                    state.mr_selected[idx] = mr_idx;
                    send_msg(
                        framed,
                        ClientMsg::SelectMr {
                            project_idx: idx,
                            mr_iid: iid,
                        },
                    )
                    .await?;
                }
            } else {
                state.notify(format!("Project '{}' not in config", project_name));
            }
        }
        None => {
            state.notify("No navigation target for this activity");
        }
    }
    Ok(())
}

async fn send_msg(
    framed: &mut Framed<UnixStream, LengthDelimitedCodec>,
    msg: ClientMsg,
) -> Result<()> {
    framed.send(Bytes::from(serde_json::to_vec(&msg)?)).await?;
    Ok(())
}

fn show_connection_error(sock_path: &std::path::Path) {
    use ratatui::layout::{Alignment, Constraint, Layout};
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

    let _ = enable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(stdout, EnterAlternateScreen);
    let backend = CrosstermBackend::new(stdout);
    let Ok(mut terminal) = Terminal::new(backend) else {
        return;
    };

    let _ = terminal.draw(|frame| {
        let area = frame.area();
        let vertical = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(9),
            Constraint::Fill(1),
        ])
        .split(area);
        let horizontal = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(52),
            Constraint::Fill(1),
        ])
        .split(vertical[1]);
        let rect = horizontal[1];

        let accent = Color::Rgb(255, 140, 0);
        let block = Block::default()
            .title(Line::from(Span::styled(
                " pertmux ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            )))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent));

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "daemon is not running",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("start with: ", Style::default().fg(Color::DarkGray)),
                Span::styled("pertmux serve", Style::default().fg(accent)),
            ]),
            Line::from(Span::styled(
                format!("socket:     {}", sock_path.display()),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "press any key to close",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let paragraph = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, rect);
    });

    let _ = crossterm::event::read();

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
}
