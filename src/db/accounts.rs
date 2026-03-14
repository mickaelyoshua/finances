use std::collections::HashMap;

use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{Account, AccountType};

pub async fn list_accounts(pool: &PgPool) -> Result<Vec<Account>, sqlx::Error> {
    sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE active = TRUE ORDER BY id")
        .fetch_all(pool)
        .await
}

pub async fn get_account(pool: &PgPool, id: i32) -> Result<Account, sqlx::Error> {
    sqlx::query_as::<_, Account>("SELECT * FROM accounts WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn create_account(
    pool: &PgPool,
    name: &str,
    account_type: AccountType,
    has_credit_card: bool,
    credit_limit: Option<Decimal>,
    billing_day: Option<i16>,
    due_day: Option<i16>,
    has_debit_card: bool,
) -> Result<Account, sqlx::Error> {
    sqlx::query_as::<_, Account>(
        "INSERT INTO accounts (name, account_type, has_credit_card, credit_limit, billing_day, due_day, has_debit_card)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING *",
    )
    .bind(name)
    .bind(account_type.as_str())
    .bind(has_credit_card)
    .bind(credit_limit)
    .bind(billing_day)
    .bind(due_day)
    .bind(has_debit_card)
    .fetch_one(pool)
    .await
}

pub async fn update_account(
    pool: &PgPool,
    id: i32,
    name: &str,
    account_type: AccountType,
    has_credit_card: bool,
    credit_limit: Option<Decimal>,
    billing_day: Option<i16>,
    due_day: Option<i16>,
    has_debit_card: bool,
) -> Result<Account, sqlx::Error> {
    sqlx::query_as::<_, Account>(
        "UPDATE accounts
         SET name = $2, account_type = $3, has_credit_card = $4, credit_limit = $5,
             billing_day = $6, due_day = $7, has_debit_card = $8
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(account_type.as_str())
    .bind(has_credit_card)
    .bind(credit_limit)
    .bind(billing_day)
    .bind(due_day)
    .bind(has_debit_card)
    .fetch_one(pool)
    .await
}

pub async fn deactivate_account(pool: &PgPool, id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE accounts SET active = FALSE WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Compute balance for a checking/cash account:
/// non-credit incomes − non-credit expenses + transfers in − transfers out − credit card payments
///
/// Credit income (e.g. refunds) reduces the credit card debt, not the checking balance,
/// so it is excluded here just like credit expenses.
pub async fn compute_balance(pool: &PgPool, account_id: i32) -> Result<Decimal, sqlx::Error> {
    let row: (Decimal,) = sqlx::query_as(
        "SELECT
            COALESCE((SELECT SUM(amount) FROM transactions WHERE account_id = $1 AND transaction_type = 'income' AND payment_method != 'credit'), 0)
            - COALESCE((SELECT SUM(amount) FROM transactions WHERE account_id = $1 AND transaction_type = 'expense' AND payment_method != 'credit'), 0)
            + COALESCE((SELECT SUM(amount) FROM transfers WHERE to_account_id = $1), 0)
            - COALESCE((SELECT SUM(amount) FROM transfers WHERE from_account_id = $1), 0)
            - COALESCE((SELECT SUM(amount) FROM credit_card_payments WHERE account_id = $1), 0)",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Compute credit card used amount for an account that has_credit_card:
/// credit expenses − credit income (refunds) − credit card payments
pub async fn compute_credit_used(pool: &PgPool, account_id: i32) -> Result<Decimal, sqlx::Error> {
    let row: (Decimal,) = sqlx::query_as(
        "SELECT
            COALESCE((SELECT SUM(amount) FROM transactions WHERE account_id = $1 AND payment_method = 'credit' AND transaction_type = 'expense'), 0)
            - COALESCE((SELECT SUM(amount) FROM transactions WHERE account_id = $1 AND payment_method = 'credit' AND transaction_type = 'income'), 0)
            - COALESCE((SELECT SUM(amount) FROM credit_card_payments WHERE account_id = $1), 0)",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Compute checking balance and credit used for all active accounts in a single query.
pub async fn compute_all_balances(
    pool: &PgPool,
) -> Result<HashMap<i32, (Decimal, Decimal)>, sqlx::Error> {
    let rows: Vec<(i32, Decimal, Decimal)> = sqlx::query_as(
        "SELECT
            a.id,
            COALESCE((SELECT SUM(amount) FROM transactions WHERE account_id = a.id AND transaction_type = 'income' AND payment_method != 'credit'), 0)
            - COALESCE((SELECT SUM(amount) FROM transactions WHERE account_id = a.id AND transaction_type = 'expense' AND payment_method != 'credit'), 0)
            + COALESCE((SELECT SUM(amount) FROM transfers WHERE to_account_id = a.id), 0)
            - COALESCE((SELECT SUM(amount) FROM transfers WHERE from_account_id = a.id), 0)
            - COALESCE((SELECT SUM(amount) FROM credit_card_payments WHERE account_id = a.id), 0),
            CASE WHEN a.has_credit_card THEN
                COALESCE((SELECT SUM(amount) FROM transactions WHERE account_id = a.id AND payment_method = 'credit' AND transaction_type = 'expense'), 0)
                - COALESCE((SELECT SUM(amount) FROM transactions WHERE account_id = a.id AND payment_method = 'credit' AND transaction_type = 'income'), 0)
                - COALESCE((SELECT SUM(amount) FROM credit_card_payments WHERE account_id = a.id), 0)
            ELSE 0
            END
        FROM accounts a
        WHERE a.active = TRUE
        ORDER BY a.id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id, c, u)| (id, (c, u))).collect())
}

/// Check if an account is referenced by any transactions, transfers,
/// credit card payments, installment purchases, or recurring transactions.
pub async fn has_references(pool: &PgPool, id: i32) -> Result<bool, sqlx::Error> {
    let row: (bool,) = sqlx::query_as(
        "SELECT
            EXISTS(SELECT 1 FROM transactions WHERE account_id = $1)
            OR EXISTS(SELECT 1 FROM transfers WHERE from_account_id = $1 OR to_account_id = $1)
            OR EXISTS(SELECT 1 FROM credit_card_payments WHERE account_id = $1)
            OR EXISTS(SELECT 1 FROM installment_purchases WHERE account_id = $1)
            OR EXISTS(SELECT 1 FROM recurring_transactions WHERE account_id = $1)",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Return the set of distinct payment methods used in transactions for an account.
pub async fn used_payment_methods(pool: &PgPool, account_id: i32) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT payment_method FROM transactions WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(m,)| m).collect())
}

/// Load (id, name) pairs for ALL accounts (including inactive) for display lookups.
pub async fn list_all_account_names(pool: &PgPool) -> Result<HashMap<i32, String>, sqlx::Error> {
    let rows: Vec<(i32, String)> =
        sqlx::query_as("SELECT id, name FROM accounts ORDER BY id")
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().collect())
}
