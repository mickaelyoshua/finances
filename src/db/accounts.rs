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
    has_credit_card: bool,
    credit_limit: Option<Decimal>,
    billing_day: Option<i16>,
    due_day: Option<i16>,
    has_debit_card: bool,
) -> Result<Account, sqlx::Error> {
    sqlx::query_as::<_, Account>(
        "UPDATE accounts
         SET name = $2, has_credit_card = $3, credit_limit = $4, billing_day = $5, due_day = $6, has_debit_card = $7
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(name)
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
/// incomes - non-credit expenses + transfers_in - transfers_out - credit_card_payments
pub async fn compute_balance(pool: &PgPool, account_id: i32) -> Result<Decimal, sqlx::Error> {
    let row: (Decimal,) = sqlx::query_as(
        "SELECT
            COALESCE((SELECT SUM(amount) FROM transactions WHERE account_id = $1 AND transaction_type = 'income'), 0)
            -- Credit expenses don't leave the checking account; they accumulate
            -- on the card and are settled via credit_card_payments instead.
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
/// credit expenses - credit card payments made on this account
pub async fn compute_credit_used(pool: &PgPool, account_id: i32) -> Result<Decimal, sqlx::Error> {
    let row: (Decimal,) = sqlx::query_as(
        "SELECT
            COALESCE((SELECT SUM(amount) FROM transactions WHERE account_id = $1 AND payment_method = 'credit'), 0)
            - COALESCE((SELECT SUM(amount) FROM credit_card_payments WHERE account_id = $1), 0)",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}
