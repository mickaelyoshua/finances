//! Database operations for the finances app.
//!
//! # Data model
//! An **account** represents a financial institution (Nubank, PicPay) or cash wallet.
//! Each account can optionally have a credit card and/or debit card.
//! Balances are always computed from transactions — never stored directly.
//!
//! # Balance computation
//! - **Checking balance** = incomes − non-credit expenses + transfers in − transfers out − credit card payments
//! - **Credit card used** = credit expenses − credit card payments
//!
//! Credit card bill payments use the `credit_card_payments` table (not transfers),
//! because checking and credit card live on the same account — a self-transfer is impossible.

pub mod accounts;
pub mod budgets;
pub mod categories;
pub mod credit_card_payments;
pub mod installments;
pub mod notifications;
pub mod recurring;
pub mod transactions;
pub mod transfers;

use chrono::{Datelike, NaiveDate};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// Add months to a date, preserving the original day when possible.
/// Falls back to the last day of the target month when the day doesn't exist
/// (e.g., Jan 31 + 1 month = Feb 28).
pub(crate) fn add_months(date: NaiveDate, months: u32) -> NaiveDate {
    let total_months = date.year() as i32 * 12 + date.month0() as i32 + months as i32;
    let year = total_months / 12;
    let month = (total_months % 12) as u32 + 1;
    let day = date.day().min(last_day_of_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .unwrap()
    .pred_opt()
    .unwrap()
    .day()
}
