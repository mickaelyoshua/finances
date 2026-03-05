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
