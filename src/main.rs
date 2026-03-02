mod app;
mod coding_agent;
mod config;
mod db;
mod discovery;
mod tmux;
mod types;
mod ui;

use app::App;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::time::Duration;

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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = config::load(cli.config.as_deref())?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);
    app.refresh();

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

fn run_loop(terminal: &mut Terminal<impl Backend>, app: &mut App) -> anyhow::Result<()> {
    while app.running {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press {
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

        if app.should_refresh() {
            app.refresh();
        }
    }
    Ok(())
}
