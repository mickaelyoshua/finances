use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{Frequency, PaymentMethod, RecurringTransaction, TransactionType};

pub struct RecurringParams {
    pub amount: Decimal,
    pub description: String,
    pub category_id: i32,
    pub account_id: i32,
    pub transaction_type: TransactionType,
    pub payment_method: PaymentMethod,
    pub frequency: Frequency,
    pub next_due: NaiveDate,
}

pub async fn list_recurring(pool: &PgPool) -> Result<Vec<RecurringTransaction>, sqlx::Error> {
    sqlx::query_as::<_, RecurringTransaction>(
        "SELECT * FROM recurring_transactions WHERE active = TRUE ORDER BY next_due",
    )
    .fetch_all(pool)
    .await
}

pub async fn list_pending(
    pool: &PgPool,
    today: NaiveDate,
) -> Result<Vec<RecurringTransaction>, sqlx::Error> {
    sqlx::query_as::<_, RecurringTransaction>(
        "SELECT * FROM recurring_transactions WHERE active = TRUE AND next_due <= $1 ORDER BY next_due",
    )
    .bind(today)
    .fetch_all(pool)
    .await
}

pub async fn create_recurring(
    pool: &PgPool,
    params: &RecurringParams,
) -> Result<RecurringTransaction, sqlx::Error> {
    sqlx::query_as::<_, RecurringTransaction>(
        "INSERT INTO recurring_transactions
            (amount, description, category_id, account_id, transaction_type, payment_method, frequency, next_due)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING *",
    )
    .bind(params.amount)
    .bind(&params.description)
    .bind(params.category_id)
    .bind(params.account_id)
    .bind(params.transaction_type.as_str())
    .bind(params.payment_method.as_str())
    .bind(params.frequency.as_str())
    .bind(params.next_due)
    .fetch_one(pool)
    .await
}

pub async fn advance_next_due(
    pool: &PgPool,
    id: i32,
    new_next_due: NaiveDate,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE recurring_transactions SET next_due = $2 WHERE id = $1")
        .bind(id)
        .bind(new_next_due)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_recurring(
    pool: &PgPool,
    id: i32,
    params: &RecurringParams,
) -> Result<RecurringTransaction, sqlx::Error> {
    sqlx::query_as::<_, RecurringTransaction>(
        "UPDATE recurring_transactions
         SET amount = $2, description = $3, category_id = $4, account_id = $5,
             transaction_type = $6, payment_method = $7, frequency = $8, next_due = $9
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(params.amount)
    .bind(&params.description)
    .bind(params.category_id)
    .bind(params.account_id)
    .bind(params.transaction_type.as_str())
    .bind(params.payment_method.as_str())
    .bind(params.frequency.as_str())
    .bind(params.next_due)
    .fetch_one(pool)
    .await
}

pub async fn deactivate_recurring(pool: &PgPool, id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE recurring_transactions SET active = FALSE WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Compute the next due date based on frequency
pub fn compute_next_due(current: NaiveDate, frequency: Frequency) -> NaiveDate {
    match frequency {
        Frequency::Daily => current + chrono::TimeDelta::days(1),
        Frequency::Weekly => current + chrono::TimeDelta::weeks(1),
        Frequency::Monthly => super::add_months(current, 1),
        Frequency::Yearly => super::add_months(current, 12),
    }
}
