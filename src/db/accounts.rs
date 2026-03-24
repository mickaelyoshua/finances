use std::collections::HashMap;

use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{Account, AccountType};

pub struct AccountParams {
    pub name: String,
    pub account_type: AccountType,
    pub has_credit_card: bool,
    pub credit_limit: Option<Decimal>,
    pub billing_day: Option<i16>,
    pub due_day: Option<i16>,
    pub has_debit_card: bool,
}

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

pub async fn create_account(pool: &PgPool, params: &AccountParams) -> Result<Account, sqlx::Error> {
    sqlx::query_as::<_, Account>(
        "INSERT INTO accounts (name, account_type, has_credit_card, credit_limit, billing_day, due_day, has_debit_card)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING *",
    )
    .bind(&params.name)
    .bind(params.account_type.as_str())
    .bind(params.has_credit_card)
    .bind(params.credit_limit)
    .bind(params.billing_day)
    .bind(params.due_day)
    .bind(params.has_debit_card)
    .fetch_one(pool)
    .await
}

pub async fn update_account(
    pool: &PgPool,
    id: i32,
    params: &AccountParams,
) -> Result<Account, sqlx::Error> {
    sqlx::query_as::<_, Account>(
        "UPDATE accounts
         SET name = $2, account_type = $3, has_credit_card = $4, credit_limit = $5,
             billing_day = $6, due_day = $7, has_debit_card = $8
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&params.name)
    .bind(params.account_type.as_str())
    .bind(params.has_credit_card)
    .bind(params.credit_limit)
    .bind(params.billing_day)
    .bind(params.due_day)
    .bind(params.has_debit_card)
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
///
/// Uses LEFT JOINs with pre-aggregated subqueries instead of correlated subqueries,
/// so the planner scans each table once rather than once per account.
pub async fn compute_all_balances(
    pool: &PgPool,
) -> Result<HashMap<i32, (Decimal, Decimal)>, sqlx::Error> {
    let rows: Vec<(i32, Decimal, Decimal)> = sqlx::query_as(
        "SELECT
            a.id,
            -- Checking balance: non-credit income - non-credit expense + transfers_in - transfers_out - cc_payments
            COALESCE(t.non_credit_income, 0) - COALESCE(t.non_credit_expense, 0)
                + COALESCE(tr_in.total, 0) - COALESCE(tr_out.total, 0)
                - COALESCE(ccp.total, 0),
            -- Credit used: credit expenses - credit income - cc_payments (only if has_credit_card)
            CASE WHEN a.has_credit_card THEN
                COALESCE(t.credit_expense, 0) - COALESCE(t.credit_income, 0) - COALESCE(ccp.total, 0)
            ELSE 0
            END
        FROM accounts a
        LEFT JOIN (
            SELECT account_id,
                SUM(amount) FILTER (WHERE transaction_type = 'income'  AND payment_method != 'credit') AS non_credit_income,
                SUM(amount) FILTER (WHERE transaction_type = 'expense' AND payment_method != 'credit') AS non_credit_expense,
                SUM(amount) FILTER (WHERE transaction_type = 'expense' AND payment_method  = 'credit') AS credit_expense,
                SUM(amount) FILTER (WHERE transaction_type = 'income'  AND payment_method  = 'credit') AS credit_income
            FROM transactions
            GROUP BY account_id
        ) t ON t.account_id = a.id
        LEFT JOIN (
            SELECT to_account_id AS account_id, SUM(amount) AS total
            FROM transfers GROUP BY to_account_id
        ) tr_in ON tr_in.account_id = a.id
        LEFT JOIN (
            SELECT from_account_id AS account_id, SUM(amount) AS total
            FROM transfers GROUP BY from_account_id
        ) tr_out ON tr_out.account_id = a.id
        LEFT JOIN (
            SELECT account_id, SUM(amount) AS total
            FROM credit_card_payments GROUP BY account_id
        ) ccp ON ccp.account_id = a.id
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
pub async fn used_payment_methods(
    pool: &PgPool,
    account_id: i32,
) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT DISTINCT payment_method FROM transactions WHERE account_id = $1")
            .bind(account_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(m,)| m).collect())
}

/// Load (id, name) pairs for ALL accounts (including inactive) for display lookups.
pub async fn list_all_account_names(pool: &PgPool) -> Result<HashMap<i32, String>, sqlx::Error> {
    let rows: Vec<(i32, String)> = sqlx::query_as("SELECT id, name FROM accounts ORDER BY id")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().collect())
}
