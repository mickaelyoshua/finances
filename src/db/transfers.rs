use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::Transfer;

pub async fn list_transfers(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Transfer>, sqlx::Error> {
    sqlx::query_as::<_, Transfer>(
        "SELECT * FROM transfers ORDER BY date DESC, id DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn create_transfer(
    pool: &PgPool,
    from_account_id: i32,
    to_account_id: i32,
    amount: Decimal,
    description: &str,
    date: NaiveDate,
) -> Result<Transfer, sqlx::Error> {
    sqlx::query_as::<_, Transfer>(
        "INSERT INTO transfers (from_account_id, to_account_id, amount, description, date)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(from_account_id)
    .bind(to_account_id)
    .bind(amount)
    .bind(description)
    .bind(date)
    .fetch_one(pool)
    .await
}

pub async fn count_transfers(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transfers")
        .fetch_one(pool)
        .await?;
    Ok(row.0 as u64)
}

/// Fetch all transfers (no pagination) for CSV export.
pub async fn list_all_transfers(pool: &PgPool) -> Result<Vec<Transfer>, sqlx::Error> {
    sqlx::query_as::<_, Transfer>("SELECT * FROM transfers ORDER BY date DESC, id DESC")
        .fetch_all(pool)
        .await
}

pub async fn delete_transfer(pool: &PgPool, id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM transfers WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
