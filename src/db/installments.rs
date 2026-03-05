use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{InstallmentPurchase, Transaction};

pub async fn list_installment_purchases(
    pool: &PgPool,
) -> Result<Vec<InstallmentPurchase>, sqlx::Error> {
    sqlx::query_as::<_, InstallmentPurchase>(
        "SELECT * FROM installment_purchases ORDER BY first_installment_date DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_installment_transactions(
    pool: &PgPool,
    purchase_id: i32,
) -> Result<Vec<Transaction>, sqlx::Error> {
    sqlx::query_as::<_, Transaction>(
        "SELECT * FROM transactions
         WHERE installment_purchase_id = $1
         ORDER BY installment_number",
    )
    .bind(purchase_id)
    .fetch_all(pool)
    .await
}

/// Create an installment purchase and generate all individual transactions.
/// The last installment absorbs rounding remainder.
pub async fn create_installment_purchase(
    pool: &PgPool,
    total_amount: Decimal,
    installment_count: i16,
    description: &str,
    category_id: i32,
    account_id: i32,
    first_installment_date: NaiveDate,
) -> Result<InstallmentPurchase, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let purchase = sqlx::query_as::<_, InstallmentPurchase>(
        "INSERT INTO installment_purchases
            (total_amount, installment_count, description, category_id, account_id, first_installment_date)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(total_amount)
    .bind(installment_count)
    .bind(description)
    .bind(category_id)
    .bind(account_id)
    .bind(first_installment_date)
    .fetch_one(&mut *tx)
    .await?;

    let per_installment = total_amount / Decimal::from(installment_count);
    let per_installment = per_installment.round_dp(2);

    for i in 1..=installment_count {
        let amount = if i == installment_count {
            total_amount - per_installment * Decimal::from(installment_count - 1)
        } else {
            per_installment
        };

        let date = super::add_months(first_installment_date, (i - 1) as u32);
        let desc = format!("{} ({}/{})", description, i, installment_count);

        // Installments are always credit card expenses by definition
        sqlx::query(
            "INSERT INTO transactions
                (amount, description, category_id, account_id, transaction_type, payment_method, date, installment_purchase_id, installment_number)
             VALUES ($1, $2, $3, $4, 'expense', 'credit', $5, $6, $7)",
        )
        .bind(amount)
        .bind(&desc)
        .bind(category_id)
        .bind(account_id)
        .bind(date)
        .bind(purchase.id)
        .bind(i)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(purchase)
}

pub async fn delete_installment_purchase(pool: &PgPool, id: i32) -> Result<(), sqlx::Error> {
    // Transactions are deleted via ON DELETE CASCADE
    sqlx::query("DELETE FROM installment_purchases WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
