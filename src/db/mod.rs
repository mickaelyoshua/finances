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
    let max_retries = 3;
    let mut last_err = None;
    for attempt in 1..=max_retries {
        match PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(60))
            .connect(database_url)
            .await
        {
            Ok(pool) => {
                // Verify the connection is actually alive
                match sqlx::query("SELECT 1").execute(&pool).await {
                    Ok(_) => return Ok(pool),
                    Err(e) => {
                        tracing::warn!(
                            "connection attempt {attempt}/{max_retries} connected but ping failed: {e}"
                        );
                        last_err = Some(e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("connection attempt {attempt}/{max_retries} failed: {e}");
                last_err = Some(e);
            }
        }
        if attempt < max_retries {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }
    Err(last_err.unwrap())
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// Add months to a date, preserving the original day when possible.
/// Falls back to the last day of the target month when the day doesn't exist
/// (e.g., Jan 31 + 1 month = Feb 28).
pub(crate) fn add_months(date: NaiveDate, months: u32) -> NaiveDate {
    let total_months = date.year() * 12 + date.month0() as i32 + months as i32;
    let year = total_months / 12;
    let month = (total_months % 12) as u32 + 1;
    let day = date.day().min(last_day_of_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

pub fn last_day_of_month(year: i32, month: u32) -> u32 {
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

pub fn prev_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

pub fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

/// Clamp a day number to the last valid day of a given year/month.
/// e.g., day=31, month=February → 28 (or 29 in leap year).
pub fn clamped_day(year: i32, month: u32, day: u32) -> NaiveDate {
    let last = last_day_of_month(year, month);
    NaiveDate::from_ymd_opt(year, month, day.min(last)).unwrap()
}

/// Find the most recent statement closing date on or before `today`.
pub fn latest_closing_date(today: NaiveDate, billing_day: u32) -> NaiveDate {
    let close = clamped_day(today.year(), today.month(), billing_day);
    if close <= today {
        close
    } else {
        let (y, m) = prev_month(today.year(), today.month());
        clamped_day(y, m, billing_day)
    }
}

/// Compute the statement period (start inclusive, end inclusive) for a given
/// closing date and billing_day.
/// Start = previous month's billing_day + 1, End = this month's billing_day.
pub fn statement_period(close_date: NaiveDate, billing_day: u32) -> (NaiveDate, NaiveDate) {
    let end = close_date;
    let (py, pm) = prev_month(close_date.year(), close_date.month());
    let prev_close = clamped_day(py, pm, billing_day);
    let start = prev_close.succ_opt().unwrap();
    (start, end)
}

/// Compute the due date for a statement closing in a given year/month.
/// If due_day > billing_day, due date is in the same month as closing.
/// If due_day <= billing_day, due date is in the following month.
pub fn statement_due_date(year: i32, month: u32, billing_day: u32, due_day: u32) -> NaiveDate {
    if due_day > billing_day {
        clamped_day(year, month, due_day)
    } else {
        let (ny, nm) = next_month(year, month);
        clamped_day(ny, nm, due_day)
    }
}
