//! Transaction CRUD with dynamic filtering via [`QueryBuilder`].
//!
//! The filter system uses `WHERE TRUE` as a base so every clause can
//! unconditionally prepend `AND`. Description search uses `ILIKE` with
//! escaped wildcards (`%`, `_`, `\`) to prevent SQL pattern injection.
//!
//! Batch helpers ([`sum_credit_by_accounts_batch`]) use a `VALUES` list
//! joined against the transactions table to compute per-account credit
//! totals in a single round trip, avoiding N+1 queries.

use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{PgPool, Postgres, QueryBuilder};

use tracing::debug;

use crate::models::{PaymentMethod, Transaction, TransactionType};

pub struct TransactionParams {
    pub amount: Decimal,
    pub description: String,
    pub category_id: i32,
    pub account_id: i32,
    pub transaction_type: TransactionType,
    pub payment_method: PaymentMethod,
    pub date: NaiveDate,
}

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
        let escaped = desc
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        qb.push(" AND description ILIKE ")
            .push_bind(format!("%{escaped}%"));
    }
}

pub async fn create_transaction(
    pool: &PgPool,
    params: &TransactionParams,
) -> Result<Transaction, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING *",
    )
    .bind(params.amount)
    .bind(&params.description)
    .bind(params.category_id)
    .bind(params.account_id)
    .bind(params.transaction_type.as_str())
    .bind(params.payment_method.as_str())
    .bind(params.date)
    .fetch_one(pool)
    .await
}

pub async fn update_transaction(
    pool: &PgPool,
    id: i32,
    params: &TransactionParams,
) -> Result<Transaction, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "UPDATE transactions
         SET amount = $2, description = $3, category_id = $4, account_id = $5,
             transaction_type = $6, payment_method = $7, date = $8
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
    .bind(params.date)
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
    let row: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM transactions WHERE date = $1 AND installment_purchase_id IS NULL)")
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

/// Fetch all credit card transactions for an account within a date range.
pub async fn list_credit_by_account(
    pool: &PgPool,
    account_id: i32,
    date_from: NaiveDate,
    date_to: NaiveDate,
) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT * FROM transactions
         WHERE account_id = $1
           AND payment_method = 'credit'
           AND date BETWEEN $2 AND $3
         ORDER BY date DESC, id DESC",
    )
    .bind(account_id)
    .bind(date_from)
    .bind(date_to)
    .fetch_all(pool)
    .await
}

/// Latest credit transaction date for an account (used to determine future statements range).
pub async fn max_credit_date(
    pool: &PgPool,
    account_id: i32,
) -> Result<Option<NaiveDate>, sqlx::Error> {
    let row: (Option<NaiveDate>,) = sqlx::query_as(
        "SELECT MAX(date) FROM transactions
         WHERE account_id = $1 AND payment_method = 'credit'",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Sum credit card expenses and incomes for an account in a date range.
/// Returns (total_expenses, total_incomes).
pub async fn sum_credit_by_account_in_range(
    pool: &PgPool,
    account_id: i32,
    date_from: NaiveDate,
    date_to: NaiveDate,
) -> Result<(Decimal, Decimal), sqlx::Error> {
    let row: (Option<Decimal>, Option<Decimal>) = sqlx::query_as(
        "SELECT
            SUM(CASE WHEN transaction_type = 'expense' THEN amount ELSE 0 END),
            SUM(CASE WHEN transaction_type = 'income' THEN amount ELSE 0 END)
         FROM transactions
         WHERE account_id = $1
           AND payment_method = 'credit'
           AND date BETWEEN $2 AND $3",
    )
    .bind(account_id)
    .bind(date_from)
    .bind(date_to)
    .fetch_one(pool)
    .await?;
    Ok((row.0.unwrap_or_default(), row.1.unwrap_or_default()))
}

/// Batch sum credit card expenses and incomes for multiple accounts, each with its own date range.
/// Returns a HashMap from account_id to (total_expenses, total_incomes).
///
/// Uses a single query with a VALUES list joined against transactions, avoiding N+1.
pub async fn sum_credit_by_accounts_batch(
    pool: &PgPool,
    ranges: &[(i32, NaiveDate, NaiveDate)],
) -> Result<HashMap<i32, (Decimal, Decimal)>, sqlx::Error> {
    use std::collections::HashMap;

    if ranges.is_empty() {
        return Ok(HashMap::new());
    }

    let mut qb = QueryBuilder::<Postgres>::new(
        "SELECT v.account_id,
            COALESCE(SUM(CASE WHEN t.transaction_type = 'expense' THEN t.amount END), 0),
            COALESCE(SUM(CASE WHEN t.transaction_type = 'income'  THEN t.amount END), 0)
         FROM (VALUES ",
    );
    for (i, (account_id, date_from, date_to)) in ranges.iter().enumerate() {
        if i > 0 {
            qb.push(", ");
        }
        qb.push("(");
        qb.push_bind(*account_id);
        qb.push("::int, ");
        qb.push_bind(*date_from);
        qb.push("::date, ");
        qb.push_bind(*date_to);
        qb.push("::date)");
    }
    qb.push(
        ") AS v(account_id, date_from, date_to) \
         LEFT JOIN transactions t ON t.account_id = v.account_id \
             AND t.payment_method = 'credit' \
             AND t.date BETWEEN v.date_from AND v.date_to \
         GROUP BY v.account_id",
    );

    let rows: Vec<(i32, Decimal, Decimal)> = qb.build_query_as().fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(id, e, i)| (id, (e, i))).collect())
}
