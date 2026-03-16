use clap::Parser;

#[derive(Parser)]
#[command(name = "finances", about = "Personal finance TUI manager")]
pub struct Config {
    /// Run database migrations and exit
    #[arg(long)]
    pub migrate: bool,

    /// Check for alerts (no transactions today, overdue recurring, over-budget) and send a desktop notification
    #[arg(long)]
    pub notify: bool,

    /// Connect to the production database (.env.prod) instead of local dev (.env)
    #[arg(long)]
    pub prod: bool,
}

pub fn database_url(prod: bool) -> String {
    let env_file = if prod { ".env.prod" } else { ".env" };

    dotenvy::from_filename(env_file)
        .unwrap_or_else(|_| panic!("{env_file} not found — copy {env_file}.example and fill in your credentials"));

    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set in {env_file}"))
}
