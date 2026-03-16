use sqlx::PgPool;

use crate::models::{Notification, NotificationType};

/// Insert a notification unless an unread one with the same (type, reference_id) already exists.
///
/// Relies on the `idx_notifications_dedup` partial unique index (covers only `read = FALSE` rows).
/// `ON CONFLICT DO NOTHING` makes duplicate inserts a silent no-op instead of an error.
/// Once the user marks a notification as read, the index no longer blocks it, so the next
/// `--notify` run can create a fresh one if the condition persists.
pub async fn insert_if_new(
    pool: &PgPool,
    message: &str,
    notification_type: NotificationType,
    reference_id: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO notifications (message, notification_type, reference_id)
         VALUES ($1, $2, $3)
         ON CONFLICT (notification_type, COALESCE(reference_id, 0))
             WHERE read = FALSE
         DO NOTHING",
    )
    .bind(message)
    .bind(notification_type.as_str())
    .bind(reference_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_unread(pool: &PgPool) -> Result<Vec<Notification>, sqlx::Error> {
    sqlx::query_as::<_, Notification>(
        "SELECT * FROM notifications WHERE read = FALSE ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn mark_read(pool: &PgPool, id: i32) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE notifications SET read = TRUE WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_all_read(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE notifications SET read = TRUE WHERE read = FALSE")
        .execute(pool)
        .await?;
    Ok(())
}
