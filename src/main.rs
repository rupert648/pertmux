mod app;
mod coding_agent;
mod client;
mod config;
mod daemon;
mod db;
mod discovery;
mod git;
#[allow(unused)]
mod gitlab;
mod linking;
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
    about = "TUI dashboard for coding agent sessions in tmux"
)]
struct Cli {
    #[arg(short = 'c', long = "config")]
    config: Option<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Run as background daemon")]
    Serve,
    #[command(about = "Stop the running daemon")]
    Stop,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Serve) => {
            let config = config::load(cli.config.as_deref())?;
            daemon::run(config).await
        }
        Some(Commands::Stop) => client::stop().await,
        None => client::run(cli.config.as_deref()).await,
    }
}
