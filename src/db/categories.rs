use sqlx::PgPool;

use crate::models::{Category, CategoryType};

pub async fn list_categories(pool: &PgPool) -> Result<Vec<Category>, sqlx::Error> {
    sqlx::query_as::<_, Category>("SELECT * FROM categories ORDER BY category_type, name")
        .fetch_all(pool)
        .await
}

pub async fn list_by_type(
    pool: &PgPool,
    category_type: CategoryType,
) -> Result<Vec<Category>, sqlx::Error> {
    sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE category_type = $1 ORDER BY name")
        .bind(category_type.as_str())
        .fetch_all(pool)
        .await
}

pub async fn create_category(
    pool: &PgPool,
    name: &str,
    category_type: CategoryType,
) -> Result<Category, sqlx::Error> {
    sqlx::query_as::<_, Category>(
        "INSERT INTO categories (name, category_type) VALUES ($1, $2) RETURNING *",
    )
    .bind(name)
    .bind(category_type.as_str())
    .fetch_one(pool)
    .await
}

pub async fn update_category(
    pool: &PgPool,
    id: i32,
    name: &str,
    category_type: CategoryType,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE categories SET name = $1, category_type = $2 WHERE id = $3")
        .bind(name)
        .bind(category_type.as_str())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_category(pool: &PgPool, id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM categories WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Check if a category is referenced by any transactions, budgets,
/// installment purchases, or recurring transactions.
pub async fn has_references(pool: &PgPool, id: i32) -> Result<bool, sqlx::Error> {
    let row: (bool,) = sqlx::query_as(
        "SELECT
            EXISTS(SELECT 1 FROM transactions WHERE category_id = $1)
            OR EXISTS(SELECT 1 FROM budgets WHERE category_id = $1)
            OR EXISTS(SELECT 1 FROM installment_purchases WHERE category_id = $1)
            OR EXISTS(SELECT 1 FROM recurring_transactions WHERE category_id = $1)",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}
