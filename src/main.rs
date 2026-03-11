mod config;
mod db;
mod models;
mod ui;

use std::time::Duration;

use chrono::Local;
use clap::Parser;
use config::Config;
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::{Terminal, backend::CrosstermBackend};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::parse();
    let database_url = config::database_url();
    let pool = db::create_pool(&database_url).await?;

    if cfg.migrate {
        println!("Running migrations...");
        db::run_migrations(&pool).await?;
        println!("Migrations complete.");
        return Ok(());
    }

    if cfg.notify {
        let today = Local::now().date_naive();
        let has_entries = db::transactions::has_transactions_today(&pool, today).await?;
        if !has_entries {
            notify_rust::Notification::new()
                .summary("Finances")
                .body("You haven't logged any transactions today!")
                .show()?;
        }
        return Ok(());
    }

    // -- Terminal setup --
    terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    // -- App init --
    let mut app = ui::App::new(pool);
    app.load_data().await?;
    let mut events = ui::EventHandler::new(Duration::from_millis(250));

    // -- Main loop --
    let result = run_loop(&mut terminal, &mut app, &mut events).await;

    // -- Terminal teardown (always run) --
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut ui::App,
    events: &mut ui::EventHandler,
) -> anyhow::Result<()> {
    while app.running {
        terminal.draw(|frame| ui::render::draw(frame, app))?;
        match events.next().await {
            Some(ui::AppEvent::Key(key)) => app.handle_key(key).await?,
            Some(ui::AppEvent::Resize(_, _)) | Some(ui::AppEvent::Tick) => {}
            None => break,
        }
    }
    Ok(())
}
