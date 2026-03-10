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
    Serve,
    #[command(about = "Connect TUI client to running daemon")]
    Connect,
    #[command(about = "Stop the running daemon")]
    Stop,
    #[command(about = "Show daemon status and socket info")]
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve => {
            let config = config::load(cli.config.as_deref())?;
            daemon::run(config).await
        }
        Commands::Connect => client::run().await,
        Commands::Stop => client::stop().await,
        Commands::Status => {
            client::status();
            Ok(())
        }
    }
}
