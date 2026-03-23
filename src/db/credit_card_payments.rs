//! Credit card bill payments within the same account.
//!
//! A credit card and its checking account belong to the same `Account` row,
//! so a transfer (which requires two *different* accounts) cannot represent
//! paying off a card balance.  This table fills that gap — each row reduces
//! both the checking balance and the outstanding credit-card debt.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::CreditCardPayment;

pub async fn list_all_payments(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<CreditCardPayment>, sqlx::Error> {
    sqlx::query_as::<_, CreditCardPayment>(
        "SELECT * FROM credit_card_payments
         ORDER BY date DESC, id DESC
         LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn list_by_account(
    pool: &PgPool,
    account_id: i32,
    limit: i64,
    offset: i64,
) -> Result<Vec<CreditCardPayment>, sqlx::Error> {
    sqlx::query_as::<_, CreditCardPayment>(
        "SELECT * FROM credit_card_payments
         WHERE account_id = $1
         ORDER BY date DESC, id DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(account_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn create_payment(
    pool: &PgPool,
    account_id: i32,
    amount: Decimal,
    date: NaiveDate,
    description: &str,
) -> Result<CreditCardPayment, sqlx::Error> {
    sqlx::query_as::<_, CreditCardPayment>(
        "INSERT INTO credit_card_payments (account_id, amount, date, description)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(account_id)
    .bind(amount)
    .bind(date)
    .bind(description)
    .fetch_one(pool)
    .await
}

pub async fn count_payments(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM credit_card_payments")
        .fetch_one(pool)
        .await?;
    Ok(row.0 as u64)
}

/// Fetch all credit card payments (no pagination) for CSV export.
pub async fn list_all_cc_payments(pool: &PgPool) -> Result<Vec<CreditCardPayment>, sqlx::Error> {
    sqlx::query_as::<_, CreditCardPayment>(
        "SELECT * FROM credit_card_payments ORDER BY date DESC, id DESC",
    )
    .fetch_all(pool)
    .await
}

/// Fetch credit card payments for an account within a date range.
pub async fn list_payments_in_range(
    pool: &PgPool,
    account_id: i32,
    date_from: NaiveDate,
    date_to: NaiveDate,
) -> Result<Vec<CreditCardPayment>, sqlx::Error> {
    sqlx::query_as::<_, CreditCardPayment>(
        "SELECT * FROM credit_card_payments
         WHERE account_id = $1
           AND date BETWEEN $2 AND $3
         ORDER BY date DESC, id DESC",
    )
    .bind(account_id)
    .bind(date_from)
    .bind(date_to)
    .fetch_all(pool)
    .await
}

pub async fn delete_payment(pool: &PgPool, id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM credit_card_payments WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete all credit card payments for an account within a date range.
/// Used to "unpay" a statement by removing its attributed payments.
pub async fn delete_payments_in_range(
    pool: &PgPool,
    account_id: i32,
    date_from: NaiveDate,
    date_to: NaiveDate,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM credit_card_payments
         WHERE account_id = $1
           AND date BETWEEN $2 AND $3",
    )
    .bind(account_id)
    .bind(date_from)
    .bind(date_to)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
