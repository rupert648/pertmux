mod app;
mod client;
mod coding_agent;
mod config;
mod daemon;
mod db;
mod discovery;
mod forge_clients;
mod git;
mod linking;
mod mr_changes;
mod protocol;
#[allow(unused)]
mod read_state;
mod tmux;
mod types;
mod ui;
mod worktrunk;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "pertmux",
    about = "TUI dashboard for coding agent sessions in tmux",
    version,
    subcommand_required = true,
    arg_required_else_help = true
)]
struct Cli {
    #[arg(short = 'c', long = "config", global = true)]
    config: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start the background daemon")]
    Serve {
        /// Run in the foreground instead of daemonizing
        #[arg(long)]
        foreground: bool,
    },
    #[command(about = "Connect TUI client to running daemon")]
    Connect,
    #[command(about = "Stop the running daemon")]
    Stop,
    #[command(about = "Show daemon status and socket info")]
    Status,
    #[command(about = "Remove stale socket, read state, and cached data")]
    Cleanup,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { foreground } => {
            let config = config::load(cli.config.as_deref())?;
            if foreground {
                daemon::run(config).await
            } else {
                config.validate()?;
                daemonize(cli.config.as_deref())
            }
        }
        Commands::Connect => client::run().await,
        Commands::Stop => client::stop().await,
        Commands::Status => {
            client::status();
            Ok(())
        }
        Commands::Cleanup => client::cleanup(),
    }
}

fn daemonize(config_path: Option<&str>) -> anyhow::Result<()> {
    use std::fs::OpenOptions;
    use std::process::{Command, Stdio};

    let sock = daemon::socket_path();
    if sock.exists() {
        if std::os::unix::net::UnixStream::connect(&sock).is_ok() {
            anyhow::bail!(
                "another pertmux daemon is already running at {}",
                sock.display()
            );
        }
    }

    let log_path = daemon::log_path();
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let log_stderr = log_file.try_clone()?;

    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(exe);

    if let Some(cfg) = config_path {
        let abs_cfg = std::fs::canonicalize(cfg)
            .unwrap_or_else(|_| std::path::PathBuf::from(cfg));
        cmd.args(["-c", &abs_cfg.to_string_lossy()]);
    }
    cmd.args(["serve", "--foreground"]);

    cmd.stdout(log_file)
        .stderr(log_stderr)
        .stdin(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let child = cmd.spawn()?;

    eprintln!(
        "[pertmux] daemon started (pid: {}), logging to {}",
        child.id(),
        log_path.display()
    );

    Ok(())
}
