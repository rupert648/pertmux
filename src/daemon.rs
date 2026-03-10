use crate::app::App;
use crate::config::Config;
use crate::protocol::{ClientMsg, DaemonMsg, DashboardSnapshot, PROTOCOL_VERSION};
use anyhow::Result;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub fn socket_path() -> PathBuf {
    let name = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    PathBuf::from(format!("/tmp/pertmux-{}.sock", name))
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

    let latest_snapshot = Arc::new(Mutex::new(app.snapshot()));
    let _ = broadcast_tx.send(DaemonMsg::Snapshot(Box::new(app.snapshot())));

    let accept_broadcast_tx = broadcast_tx.clone();
    let accept_cmd_tx = cmd_tx.clone();
    let accept_latest_snapshot = Arc::clone(&latest_snapshot);
    tokio::spawn(async move {
        accept_loop(
            listener,
            accept_broadcast_tx,
            accept_cmd_tx,
            accept_latest_snapshot,
        )
        .await;
    });

    let mut refresh_interval = tokio::time::interval(app.refresh_interval);
    let mut detail_interval = tokio::time::interval(Duration::from_secs(60));
    let mut worktree_interval = tokio::time::interval(Duration::from_secs(30));
    refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    detail_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    worktree_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

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
                        broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
                    }
                    ClientMsg::CreateWorktree { project_idx, branch } => {
                        let result = handle_create_worktree(&app, project_idx, &branch).await;
                        send_action_result(&broadcast_tx, result);
                        app.refresh_worktrees().await;
                        broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
                    }
                    ClientMsg::RemoveWorktree { project_idx, branch } => {
                        let result = handle_remove_worktree(&app, project_idx, &branch).await;
                        send_action_result(&broadcast_tx, result);
                        app.refresh_worktrees().await;
                        broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
                    }
                    ClientMsg::MergeWorktree { project_idx, worktree_path } => {
                        let result = handle_merge_worktree(&app, project_idx, &worktree_path).await;
                        send_action_result(&broadcast_tx, result);
                        app.refresh_worktrees().await;
                        broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
                    }
                    ClientMsg::SelectMr { project_idx, mr_iid } => {
                        if let Some(proj) = app.projects.get_mut(project_idx)
                            && let Some(idx) = proj.dashboard.linked_mrs.iter().position(|l| l.mr.iid == mr_iid)
                        {
                            proj.mr_selected = idx;
                            app.active_project = project_idx;
                        }
                        app.refresh_mr_detail().await;
                        broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
                    }
                    ClientMsg::Handshake { .. } => {}
                }
            }
            _ = refresh_interval.tick() => {
                app.refresh().await;
                broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
            }
            _ = detail_interval.tick() => {
                app.refresh_mr_detail().await;
                broadcast_snapshot(&broadcast_tx, &latest_snapshot, app.snapshot()).await;
            }
            _ = worktree_interval.tick() => {
                app.refresh_worktrees().await;
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
) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let snapshot_rx = broadcast_tx.subscribe();
                let cmd_tx = cmd_tx.clone();
                let latest_snapshot = Arc::clone(&latest_snapshot);
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_client(stream, snapshot_rx, cmd_tx, latest_snapshot).await
                    {
                        let msg = e.to_string();
                        if !msg.contains("Broken pipe") && !msg.contains("Connection reset") {
                            eprintln!("[pertmux-daemon] client error: {}", e);
                        }
                    }
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
) -> Result<()> {
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    let initial_snapshot = {
        let guard = latest_snapshot.lock().await;
        guard.clone()
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
