use std::collections::HashMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{Budget, BudgetPeriod};

pub async fn list_budgets(pool: &PgPool) -> Result<Vec<Budget>, sqlx::Error> {
    sqlx::query_as::<_, Budget>("SELECT * FROM budgets ORDER BY category_id, period")
        .fetch_all(pool)
        .await
}

pub async fn create_budget(
    pool: &PgPool,
    category_id: i32,
    amount: Decimal,
    period: BudgetPeriod,
) -> Result<Budget, sqlx::Error> {
    sqlx::query_as::<_, Budget>(
        "INSERT INTO budgets (category_id, amount, period) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(category_id)
    .bind(amount)
    .bind(period.as_str())
    .fetch_one(pool)
    .await
}

pub async fn update_budget(
    pool: &PgPool,
    id: i32,
    amount: Decimal,
) -> Result<Budget, sqlx::Error> {
    sqlx::query_as::<_, Budget>("UPDATE budgets SET amount = $2 WHERE id = $1 RETURNING *")
        .bind(id)
        .bind(amount)
        .fetch_one(pool)
        .await
}

pub async fn delete_budget(pool: &PgPool, id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM budgets WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Compute current-period spending for all budgets in a single query.
/// Each budget's date range depends on its period (weekly/monthly/yearly).
pub async fn compute_all_spending(
    pool: &PgPool,
    weekly_start: NaiveDate,
    monthly_start: NaiveDate,
    yearly_start: NaiveDate,
    today: NaiveDate,
) -> Result<HashMap<i32, Decimal>, sqlx::Error> {
    let rows: Vec<(i32, Decimal)> = sqlx::query_as(
        "SELECT b.id, COALESCE(SUM(t.amount), 0)
         FROM budgets b
         LEFT JOIN transactions t ON t.category_id = b.category_id
             AND t.transaction_type = 'expense'
             AND t.date BETWEEN
                 CASE b.period
                     WHEN 'weekly' THEN $1
                     WHEN 'monthly' THEN $2
                     WHEN 'yearly' THEN $3
                 END
                 AND $4
         GROUP BY b.id",
    )
    .bind(weekly_start)
    .bind(monthly_start)
    .bind(yearly_start)
    .bind(today)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}
