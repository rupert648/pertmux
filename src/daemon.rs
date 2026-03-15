use crate::app::App;
use crate::config::Config;
use crate::mr_changes::MrChange;
use crate::protocol::{ClientMsg, DaemonMsg, DashboardSnapshot, PROTOCOL_VERSION};
use anyhow::Result;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub fn socket_path() -> PathBuf {
    let name = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    PathBuf::from(format!("/tmp/pertmux-{}.sock", name))
}

pub fn log_path() -> PathBuf {
    PathBuf::from("/tmp/pertmux-daemon.log")
}

pub async fn run(config: Config) -> Result<()> {
    let sock = socket_path();

    if sock.exists() {
        match UnixStream::connect(&sock).await {
            Ok(_) => {
                anyhow::bail!(
                    "another pertmux daemon is already running at {}",
                    sock.display()
                );
            }
            Err(_) => {
                std::fs::remove_file(&sock)?;
            }
        }
    }

    let listener = UnixListener::bind(&sock)?;
    eprintln!("[pertmux-daemon] listening on {}", sock.display());

    config.validate()?;

    let (broadcast_tx, _) = broadcast::channel::<DaemonMsg>(32);
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<ClientMsg>(64);

    let mut app = App::new(config);
    if app.has_projects() {
        app.refresh_mrs().await;
    }
    app.refresh().await;
    app.refresh_worktrees().await;
    app.pending_changes.clear();

    let latest_snapshot = Arc::new(Mutex::new(app.snapshot()));
    let _ = broadcast_tx.send(DaemonMsg::Snapshot(Box::new(app.snapshot())));

    let client_count = Arc::new(AtomicUsize::new(0));
    let pending_for_offline: Arc<Mutex<Vec<MrChange>>> = Arc::new(Mutex::new(Vec::new()));

    let accept_broadcast_tx = broadcast_tx.clone();
    let accept_cmd_tx = cmd_tx.clone();
    let accept_latest_snapshot = Arc::clone(&latest_snapshot);
    let accept_client_count = Arc::clone(&client_count);
    let accept_pending_for_offline = Arc::clone(&pending_for_offline);
    tokio::spawn(async move {
        accept_loop(
            listener,
            accept_broadcast_tx,
            accept_cmd_tx,
            accept_latest_snapshot,
            accept_client_count,
            accept_pending_for_offline,
        )
        .await;
    });

    let mut refresh_interval = tokio::time::interval(app.refresh_interval);
    let mut detail_interval = tokio::time::interval(app.mr_detail_interval);
    let mut worktree_interval = tokio::time::interval(app.worktree_interval);
    let mut mr_list_interval = tokio::time::interval(app.mr_list_interval);
    refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    detail_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    worktree_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    mr_list_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut shutdown = false;

    while !shutdown {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    ClientMsg::Stop => {
                        eprintln!("[pertmux-daemon] received stop command");
                        shutdown = true;
                    }
                    ClientMsg::Refresh => {
                        app.refresh().await;
                        app.refresh_mrs().await;
                        app.refresh_worktrees().await;
                        drain_changes(&mut app, &client_count, &pending_for_offline).await;
                        broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
                    }
                    ClientMsg::CreateWorktree { project_idx, branch } => {
                        let result = handle_create_worktree(&app, project_idx, &branch).await;
                        send_action_result(&broadcast_tx, result);
                        app.refresh_worktrees().await;
                        app.refresh().await;
                        broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
                    }
                    ClientMsg::RemoveWorktree { project_idx, branch } => {
                        let result = handle_remove_worktree(&app, project_idx, &branch).await;
                        send_action_result(&broadcast_tx, result);
                        app.refresh_worktrees().await;
                        app.refresh().await;
                        broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
                    }
                    ClientMsg::MergeWorktree { project_idx, worktree_path } => {
                        let result = handle_merge_worktree(&app, project_idx, &worktree_path).await;
                        send_action_result(&broadcast_tx, result);
                        app.refresh_worktrees().await;
                        app.refresh().await;
                        broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
                    }
                    ClientMsg::AgentAction { pane_pid, session_id, prompt } => {
                        let result = handle_agent_action(pane_pid, &session_id, &prompt).await;
                        send_action_result(&broadcast_tx, result);
                    }
                    ClientMsg::SelectMr { project_idx, mr_iid } => {
                        if let Some(proj) = app.projects.get_mut(project_idx)
                            && let Some(idx) = proj.dashboard.linked_mrs.iter().position(|l| l.mr.iid == mr_iid)
                        {
                            proj.mr_selected = idx;
                            app.active_project = project_idx;
                        }
                        app.refresh_mr_detail().await;
                        drain_changes(&mut app, &client_count, &pending_for_offline).await;
                        broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
                    }
                    ClientMsg::Handshake { .. } => {}
                }
            }
            _ = refresh_interval.tick() => {
                app.refresh().await;
                drain_changes(&mut app, &client_count, &pending_for_offline).await;
                broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
            }
            _ = detail_interval.tick() => {
                app.refresh_mr_detail().await;
                drain_changes(&mut app, &client_count, &pending_for_offline).await;
                broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
            }
            _ = worktree_interval.tick() => {
                app.refresh_worktrees().await;
                broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
            }
            _ = mr_list_interval.tick() => {
                app.refresh_mrs().await;
                drain_changes(&mut app, &client_count, &pending_for_offline).await;
                broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("[pertmux-daemon] shutting down");
                shutdown = true;
            }
        }
    }

    let _ = std::fs::remove_file(&sock);
    eprintln!("[pertmux-daemon] stopped");
    Ok(())
}

async fn accept_loop(
    listener: UnixListener,
    broadcast_tx: broadcast::Sender<DaemonMsg>,
    cmd_tx: mpsc::Sender<ClientMsg>,
    latest_snapshot: Arc<Mutex<DashboardSnapshot>>,
    client_count: Arc<AtomicUsize>,
    pending_for_offline: Arc<Mutex<Vec<MrChange>>>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let snapshot_rx = broadcast_tx.subscribe();
                let cmd_tx = cmd_tx.clone();
                let latest_snapshot = Arc::clone(&latest_snapshot);
                let client_count = Arc::clone(&client_count);
                let pending_for_offline = Arc::clone(&pending_for_offline);
                client_count.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    if let Err(e) = handle_client(
                        stream,
                        snapshot_rx,
                        cmd_tx,
                        latest_snapshot,
                        &pending_for_offline,
                    )
                    .await
                    {
                        let msg = e.to_string();
                        if !msg.contains("Broken pipe") && !msg.contains("Connection reset") {
                            eprintln!("[pertmux-daemon] client error: {}", e);
                        }
                    }
                    client_count.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(e) => {
                eprintln!("[pertmux-daemon] accept error: {}", e);
            }
        }
    }
}

async fn handle_client(
    stream: UnixStream,
    mut snapshot_rx: broadcast::Receiver<DaemonMsg>,
    cmd_tx: mpsc::Sender<ClientMsg>,
    latest_snapshot: Arc<Mutex<DashboardSnapshot>>,
    pending_for_offline: &Arc<Mutex<Vec<MrChange>>>,
) -> Result<()> {
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    let initial_snapshot = {
        let mut snapshot = {
            let guard = latest_snapshot.lock().await;
            guard.clone()
        };
        let offline_changes = {
            let mut guard = pending_for_offline.lock().await;
            std::mem::take(&mut *guard)
        };
        if !offline_changes.is_empty() {
            snapshot.pending_changes = offline_changes;
        }
        snapshot
    };
    let msg = DaemonMsg::Snapshot(Box::new(initial_snapshot));
    framed.send(Bytes::from(serde_json::to_vec(&msg)?)).await?;

    loop {
        tokio::select! {
            incoming = framed.next() => {
                match incoming {
                    Some(Ok(bytes)) => {
                        let client_msg: ClientMsg = serde_json::from_slice(&bytes)?;
                        match client_msg {
                            ClientMsg::Handshake { version } => {
                                if version != PROTOCOL_VERSION {
                                    let mismatch = DaemonMsg::ActionResult {
                                        ok: false,
                                        message: format!(
                                            "protocol version mismatch: client={}, daemon={}",
                                            version,
                                            PROTOCOL_VERSION
                                        ),
                                    };
                                    framed.send(Bytes::from(serde_json::to_vec(&mismatch)?)).await?;
                                    break;
                                }

                                let ack = DaemonMsg::HandshakeAck {
                                    version: PROTOCOL_VERSION,
                                };
                                framed.send(Bytes::from(serde_json::to_vec(&ack)?)).await?;
                            }
                            other => {
                                let _ = cmd_tx.send(other).await;
                            }
                        }
                    }
                    Some(Err(_)) => break,
                    None => break,
                }
            }
            outgoing = snapshot_rx.recv() => {
                match outgoing {
                    Ok(msg) => {
                        if framed.send(Bytes::from(serde_json::to_vec(&msg)?)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    Ok(())
}

async fn drain_changes(
    app: &mut App,
    client_count: &Arc<AtomicUsize>,
    pending_for_offline: &Arc<Mutex<Vec<MrChange>>>,
) {
    let changes = app.take_pending_changes();
    if changes.is_empty() {
        return;
    }

    if client_count.load(Ordering::SeqCst) == 0 {
        let mut guard = pending_for_offline.lock().await;
        guard.extend(changes);
    }
}

async fn handle_create_worktree(app: &App, project_idx: usize, branch: &str) -> Result<String> {
    let local_path = app
        .projects
        .get(project_idx)
        .map(|p| p.config.local_path.clone())
        .ok_or_else(|| anyhow::anyhow!("invalid project index"))?;
    crate::worktrunk::create_worktree(&local_path, branch).await
}

async fn handle_remove_worktree(app: &App, project_idx: usize, branch: &str) -> Result<String> {
    let local_path = app
        .projects
        .get(project_idx)
        .map(|p| p.config.local_path.clone())
        .ok_or_else(|| anyhow::anyhow!("invalid project index"))?;
    crate::worktrunk::remove_worktree(&local_path, branch).await
}

async fn handle_merge_worktree(
    app: &App,
    project_idx: usize,
    worktree_path: &str,
) -> Result<String> {
    if app.projects.get(project_idx).is_none() {
        anyhow::bail!("invalid project index");
    }
    crate::worktrunk::merge_worktree(worktree_path).await
}

fn send_action_result(broadcast_tx: &broadcast::Sender<DaemonMsg>, result: Result<String>) {
    let msg = match result {
        Ok(message) => DaemonMsg::ActionResult { ok: true, message },
        Err(err) => DaemonMsg::ActionResult {
            ok: false,
            message: err.to_string(),
        },
    };
    let _ = broadcast_tx.send(msg);
}

async fn handle_agent_action(pane_pid: u32, session_id: &str, prompt: &str) -> Result<String> {
    let port = crate::discovery::discover_port(pane_pid)
        .ok_or_else(|| anyhow::anyhow!("Could not discover opencode port"))?;

    let url = format!(
        "http://127.0.0.1:{}/session/{}/message",
        port, session_id
    );
    let body = serde_json::json!({
        "parts": [{"type": "text", "text": prompt}]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;

    if resp.status().is_success() {
        Ok("Message sent to opencode".to_string())
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("opencode API error ({}): {}", status, body)
    }
}

async fn broadcast_snapshot(
    broadcast_tx: &broadcast::Sender<DaemonMsg>,
    latest_snapshot: &Arc<Mutex<DashboardSnapshot>>,
    snapshot: DashboardSnapshot,
) {
    {
        let mut guard = latest_snapshot.lock().await;
        *guard = snapshot.clone();
    }
    let _ = broadcast_tx.send(DaemonMsg::Snapshot(Box::new(snapshot)));
}
