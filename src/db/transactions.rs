use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{PaymentMethod, Transaction, TransactionType};

pub async fn list_transactions(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT * FROM transactions ORDER BY date DESC, id DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn list_by_date_range(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
    limit: i64,
    offset: i64,
) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT * FROM transactions WHERE date BETWEEN $1 AND $2 ORDER BY date DESC, id DESC LIMIT $3 OFFSET $4",
    )
    .bind(from)
    .bind(to)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn list_by_account(
    pool: &PgPool,
    account_id: i32,
    from: NaiveDate,
    to: NaiveDate,
    limit: i64,
    offset: i64,
) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT * FROM transactions
         WHERE account_id = $1 AND date BETWEEN $2 AND $3
         ORDER BY date DESC, id DESC
         LIMIT $4 OFFSET $5",
    )
    .bind(account_id)
    .bind(from)
    .bind(to)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn create_transaction(
    pool: &PgPool,
    amount: Decimal,
    description: &str,
    category_id: i32,
    account_id: i32,
    transaction_type: TransactionType,
    payment_method: PaymentMethod,
    date: NaiveDate,
) -> Result<Transaction, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING *",
    )
    .bind(amount)
    .bind(description)
    .bind(category_id)
    .bind(account_id)
    .bind(transaction_type.as_str())
    .bind(payment_method.as_str())
    .bind(date)
    .fetch_one(pool)
    .await
}

pub async fn update_transaction(
    pool: &PgPool,
    id: i32,
    amount: Decimal,
    description: &str,
    category_id: i32,
    account_id: i32,
    transaction_type: TransactionType,
    payment_method: PaymentMethod,
    date: NaiveDate,
) -> Result<Transaction, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "UPDATE transactions
         SET amount = $2, description = $3, category_id = $4, account_id = $5,
             transaction_type = $6, payment_method = $7, date = $8
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(amount)
    .bind(description)
    .bind(category_id)
    .bind(account_id)
    .bind(transaction_type.as_str())
    .bind(payment_method.as_str())
    .bind(date)
    .fetch_one(pool)
    .await
}

pub async fn delete_transaction(pool: &PgPool, id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM transactions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn has_transactions_today(pool: &PgPool, today: NaiveDate) -> Result<bool, sqlx::Error> {
    let row: (bool,) =
        sqlx::query_as("SELECT EXISTS(SELECT 1 FROM transactions WHERE date = $1)")
            .bind(today)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

/// Sum expenses for a category within a date range (for budget tracking)
pub async fn sum_expenses_by_category(
    pool: &PgPool,
    category_id: i32,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Decimal, sqlx::Error> {
    let row: (Option<Decimal>,) = sqlx::query_as(
        "SELECT SUM(amount) FROM transactions
         WHERE category_id = $1 AND transaction_type = 'expense' AND date BETWEEN $2 AND $3",
    )
    .bind(category_id)
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    Ok(row.0.unwrap_or_default())
}
