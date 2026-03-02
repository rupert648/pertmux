mod app;
mod coding_agent;
mod config;
mod db;
mod discovery;
mod git;
#[allow(unused)]
mod gitlab;
#[allow(unused)]
mod read_state;
mod tmux;
mod types;
mod ui;

use app::App;
use clap::Parser;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::prelude::*;
use std::io;

#[derive(Parser)]
#[command(
    name = "pertmux",
    about = "TUI dashboard for opencode sessions in tmux"
)]
struct Cli {
    /// Path to config file
    #[arg(short = 'c', long = "config")]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = config::load(cli.config.as_deref())?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);
    app.refresh();

    let result = run_loop(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

async fn run_loop(terminal: &mut Terminal<impl Backend>, app: &mut App) -> anyhow::Result<()> {
    let mut event_stream = EventStream::new();
    let mut refresh_interval = tokio::time::interval(app.refresh_interval);
    refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    while app.running {
        terminal.draw(|frame| ui::draw(frame, app))?;

        tokio::select! {
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => app.running = false,
                            KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                            KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                            KeyCode::Enter => {
                                let _ = app.focus_selected();
                            }
                            KeyCode::Char('r') => app.refresh(),
                            _ => {}
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => {}
                    None => break,
                }
            }
            _ = refresh_interval.tick() => {
                app.refresh();
            }
        }
    }

    Ok(())
}
