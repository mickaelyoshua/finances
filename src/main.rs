use finances::config;
use finances::db;
use finances::ui;

use std::time::Duration;

use chrono::Local;
use clap::Parser;
use config::Config;
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::{Terminal, backend::CrosstermBackend};
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log file rotates daily in ~/.local/share/finances/
    let log_dir = dirs_log_dir();
    let file_appender = tracing_appender::rolling::daily(&log_dir, "finances.log");
    let _guard = init_tracing(file_appender);

    let cfg = Config::parse();
    let database_url = config::database_url(cfg.prod);

    info!("connecting to database");
    let pool = db::create_pool(&database_url).await?;

    if cfg.migrate {
        info!("running migrations");
        db::run_migrations(&pool).await?;
        info!("migrations complete");
        println!("Migrations complete.");
        return Ok(());
    }

    if cfg.notify {
        let today = Local::now().date_naive();
        let has_entries = db::transactions::has_transactions_today(&pool, today).await?;
        if !has_entries {
            info!("no transactions today, sending notification");
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
    let mut app = ui::App::new(pool, cfg.prod);
    app.load_data().await?;
    let mut events = ui::EventHandler::new(Duration::from_millis(250));

    info!("TUI started");

    // -- Main loop --
    let result = run_loop(&mut terminal, &mut app, &mut events).await;

    // -- Terminal teardown (always run) --
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    if let Err(ref e) = result {
        error!(%e, "TUI exited with error");
    } else {
        info!("TUI exited normally");
    }

    result
}

/// Returns `~/.local/share/finances/`, creating the directory if needed.
fn dirs_log_dir() -> std::path::PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("finances");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Initialise tracing with a file appender.
///
/// Returns a `WorkerGuard` that **must be held alive** for the lifetime of the
/// program — dropping it flushes and closes the log file. The guard is stored
/// in `main` as `_guard` so Rust drops it at the end of the scope.
///
/// Log level defaults to `info` and can be overridden with the `RUST_LOG`
/// environment variable (e.g. `RUST_LOG=debug`).
fn init_tracing(
    file_appender: tracing_appender::rolling::RollingFileAppender,
) -> tracing_appender::non_blocking::WorkerGuard {
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(non_blocking)
        .with_ansi(false) // no colour codes in log files
        .init();

    guard
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
