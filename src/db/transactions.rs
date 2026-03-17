use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{PgPool, Postgres, QueryBuilder};

use tracing::debug;

use crate::models::{PaymentMethod, Transaction, TransactionType};

/// Filter criteria passed from the UI filter bar to the DB query layer.
/// All fields are optional — `None` means "no constraint" for that dimension.
#[derive(Default)]
pub struct TransactionFilterParams {
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub account_id: Option<i32>,
    pub category_id: Option<i32>,
    pub transaction_type: Option<TransactionType>,
    pub payment_method: Option<PaymentMethod>,
    pub description: Option<String>,
}

pub async fn list_filtered(
    pool: &PgPool,
    filters: &TransactionFilterParams,
    limit: i64,
    offset: i64,
) -> Result<Vec<Transaction>, sqlx::Error> {
    let mut qb = QueryBuilder::new("SELECT * FROM transactions WHERE TRUE");
    push_filters(&mut qb, filters);
    qb.push(" ORDER BY date DESC, id DESC LIMIT ")
        .push_bind(limit);
    qb.push(" OFFSET ").push_bind(offset);
    let rows = qb.build_query_as::<Transaction>().fetch_all(pool).await?;
    debug!(count = rows.len(), offset, limit, "list_filtered");
    Ok(rows)
}

/// Like `list_filtered` but returns ALL matching rows (no pagination).
/// Used for CSV export.
pub async fn list_all_filtered(
    pool: &PgPool,
    filters: &TransactionFilterParams,
) -> Result<Vec<Transaction>, sqlx::Error> {
    let mut qb = QueryBuilder::new("SELECT * FROM transactions WHERE TRUE");
    push_filters(&mut qb, filters);
    qb.push(" ORDER BY date DESC, id DESC");
    let rows = qb.build_query_as::<Transaction>().fetch_all(pool).await?;
    debug!(count = rows.len(), "list_all_filtered");
    Ok(rows)
}

pub async fn count_filtered(
    pool: &PgPool,
    filters: &TransactionFilterParams,
) -> Result<u64, sqlx::Error> {
    let mut qb = QueryBuilder::new("SELECT COUNT(*) FROM transactions WHERE TRUE");
    push_filters(&mut qb, filters);
    let row: (i64,) = qb.build_query_as().fetch_one(pool).await?;
    Ok(row.0 as u64)
}

/// Append optional filter clauses to a query that starts with `WHERE TRUE`.
/// The `WHERE TRUE` base lets every filter unconditionally use `AND`.
fn push_filters(qb: &mut QueryBuilder<'_, Postgres>, filters: &TransactionFilterParams) {
    if let Some(d) = filters.date_from {
        qb.push(" AND date >= ").push_bind(d);
    }
    if let Some(d) = filters.date_to {
        qb.push(" AND date <= ").push_bind(d);
    }
    if let Some(id) = filters.account_id {
        qb.push(" AND account_id = ").push_bind(id);
    }
    if let Some(id) = filters.category_id {
        qb.push(" AND category_id = ").push_bind(id);
    }
    if let Some(t) = filters.transaction_type {
        qb.push(" AND transaction_type = ")
            .push_bind(t.as_str().to_string());
    }
    if let Some(m) = filters.payment_method {
        qb.push(" AND payment_method = ")
            .push_bind(m.as_str().to_string());
    }
    if let Some(ref desc) = filters.description {
        qb.push(" AND description ILIKE ")
            .push_bind(format!("%{desc}%"));
    }
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
    let row: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM transactions WHERE date = $1)")
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
