use clap::Parser;

#[derive(Parser)]
#[command(name = "finances", about = "Personal finance TUI manager")]
pub struct Config {
    /// Run database migrations and exit
    #[arg(long)]
    pub migrate: bool,

    /// Send a desktop notification if no transactions were entered today
    #[arg(long)]
    pub notify: bool,
}

pub fn database_url() -> String {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env or environment")
}
