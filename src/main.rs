mod config;
mod db;
mod models;

use chrono::Local;
use clap::Parser;
use config::Config;

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

    // TUI will be initialized here in Phase 2
    println!("Finances TUI — coming soon. Use --migrate to set up the database.");

    Ok(())
}
