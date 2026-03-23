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

/// Generate N installment transactions within an existing DB transaction.
/// The last installment absorbs rounding remainder.
async fn generate_installment_transactions(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    purchase: &InstallmentPurchase,
) -> Result<(), sqlx::Error> {
    let per_installment =
        (purchase.total_amount / Decimal::from(purchase.installment_count)).round_dp(2);

    for i in 1..=purchase.installment_count {
        let amount = if i == purchase.installment_count {
            purchase.total_amount
                - per_installment * Decimal::from(purchase.installment_count - 1)
        } else {
            per_installment
        };

        let date = super::add_months(purchase.first_installment_date, (i - 1) as u32);
        let desc = format!(
            "{} ({}/{})",
            purchase.description, i, purchase.installment_count
        );

        sqlx::query(
            "INSERT INTO transactions
                (amount, description, category_id, account_id, transaction_type, payment_method, date, installment_purchase_id, installment_number)
             VALUES ($1, $2, $3, $4, 'expense', 'credit', $5, $6, $7)",
        )
        .bind(amount)
        .bind(&desc)
        .bind(purchase.category_id)
        .bind(purchase.account_id)
        .bind(date)
        .bind(purchase.id)
        .bind(i)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

/// Create an installment purchase and generate all individual transactions.
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

    generate_installment_transactions(&mut tx, &purchase).await?;

    tx.commit().await?;
    Ok(purchase)
}

/// Update an installment purchase and regenerate all child transactions.
/// Deletes and recreates rather than patching because count, amount, and
/// start date may all change — diffing individual rows isn't worth it.
/// Account stays unchanged.
pub async fn update_installment_purchase(
    pool: &PgPool,
    id: i32,
    total_amount: Decimal,
    installment_count: i16,
    description: &str,
    category_id: i32,
    first_installment_date: NaiveDate,
) -> Result<InstallmentPurchase, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let purchase = sqlx::query_as::<_, InstallmentPurchase>(
        "UPDATE installment_purchases
         SET total_amount = $2, installment_count = $3, description = $4,
             category_id = $5, first_installment_date = $6
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(total_amount)
    .bind(installment_count)
    .bind(description)
    .bind(category_id)
    .bind(first_installment_date)
    .fetch_one(&mut *tx)
    .await?;

    // Delete old child transactions and regenerate
    sqlx::query("DELETE FROM transactions WHERE installment_purchase_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    generate_installment_transactions(&mut tx, &purchase).await?;

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
