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
use rust_decimal::prelude::ToPrimitive;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log file rotates daily in ~/.local/share/finances/
    let log_dir = dirs_log_dir();
    let file_appender = tracing_appender::rolling::daily(&log_dir, "finances.log");
    init_tracing(file_appender);

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
        info!("running notification checks");
        let today = Local::now().date_naive();
        let mut messages: Vec<String> = Vec::new();

        // 1. No transactions today
        let has_entries = db::transactions::has_transactions_today(&pool, today).await?;
        if !has_entries {
            let msg = "You haven't logged any transactions today!".to_string();
            db::notifications::upsert(
                &pool,
                &msg,
                finances::models::NotificationType::NoTransactions,
                None,
            )
            .await?;
            messages.push(msg);
        }

        // 2. Overdue recurring transactions
        let pending = db::recurring::list_pending(&pool, today).await?;
        for r in &pending {
            let msg = format!(
                "Overdue: {} — {}",
                r.description,
                ui::components::format::format_brl(r.amount),
            );
            db::notifications::upsert(
                &pool,
                &msg,
                finances::models::NotificationType::OverdueRecurring,
                Some(r.id),
            )
            .await?;
            messages.push(msg);
        }

        // 3. Budget alerts (50%, 75%, 90%, 100%, exceeded)
        let budgets = db::budgets::list_budgets(&pool).await?;
        let categories = db::categories::list_categories(&pool).await?;
        let (weekly_start, _) = finances::models::BudgetPeriod::Weekly.date_range(today);
        let (monthly_start, _) = finances::models::BudgetPeriod::Monthly.date_range(today);
        let (yearly_start, _) = finances::models::BudgetPeriod::Yearly.date_range(today);
        let spent_map = db::budgets::compute_all_spending(
            &pool,
            weekly_start,
            monthly_start,
            yearly_start,
            today,
        )
        .await?;

        // Ordered ascending so .rev().find() picks the highest crossed threshold per budget
        let thresholds: &[(u32, finances::models::NotificationType)] = &[
            (50, finances::models::NotificationType::Budget50),
            (75, finances::models::NotificationType::Budget75),
            (90, finances::models::NotificationType::Budget90),
            (100, finances::models::NotificationType::Budget100),
        ];

        for b in &budgets {
            let spent = spent_map.get(&b.id).copied().unwrap_or_default();
            if b.amount.is_zero() {
                continue;
            }
            let pct = (spent * rust_decimal::Decimal::from(100)) / b.amount;
            let pct_u32 = pct.to_u32().unwrap_or(0);

            let cat_name = categories
                .iter()
                .find(|c| c.id == b.category_id)
                .map(|c| c.name.as_str())
                .unwrap_or("?");
            let period = b.parsed_period().label();

            // Find the highest crossed threshold only
            if pct_u32 > 100 {
                let ntype = finances::models::NotificationType::BudgetExceeded;
                db::notifications::clear_stale_budget_notifications(&pool, b.id, ntype).await?;
                let msg = format!(
                    "Budget '{}' ({}) EXCEEDED — {}/{}",
                    cat_name,
                    period,
                    ui::components::format::format_brl(spent),
                    ui::components::format::format_brl(b.amount),
                );
                db::notifications::upsert(&pool, &msg, ntype, Some(b.id)).await?;
                messages.push(msg);
            } else if let Some(&(threshold, ntype)) =
                thresholds.iter().rev().find(|(t, _)| pct_u32 >= *t)
            {
                db::notifications::clear_stale_budget_notifications(&pool, b.id, ntype).await?;
                let msg = format!(
                    "Budget '{}' ({}) reached {}% — {}/{}",
                    cat_name,
                    period,
                    threshold,
                    ui::components::format::format_brl(spent),
                    ui::components::format::format_brl(b.amount),
                );
                db::notifications::upsert(&pool, &msg, ntype, Some(b.id)).await?;
                messages.push(msg);
            }
        }

        // Send combined desktop notification
        if !messages.is_empty() {
            let body = messages.join("\n");
            info!(count = messages.len(), "sending notifications");
            notify_rust::Notification::new()
                .summary("Finances")
                .body(&body)
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
fn init_tracing(file_appender: tracing_appender::rolling::RollingFileAppender) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(file_appender)
        .with_ansi(false) // no colour codes in log files
        .init();
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
            Some(ui::AppEvent::Tick) => app.tick(),
            Some(ui::AppEvent::Resize(_, _)) => {}
            None => break,
        }
    }
    Ok(())
}
