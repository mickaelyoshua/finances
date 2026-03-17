use sqlx::PgPool;

use crate::models::{Notification, NotificationType};

/// Insert or refresh a notification.
///
/// Relies on the `idx_notifications_dedup` partial unique index (covers only `read = FALSE` rows).
/// If an unread notification with the same (type, reference_id) already exists, the message and
/// timestamp are updated so it stays current. Once the user marks it as read, the next run
/// inserts a fresh one.
pub async fn upsert(
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
         DO UPDATE SET message = EXCLUDED.message, created_at = NOW()",
    )
    .bind(message)
    .bind(notification_type.as_str())
    .bind(reference_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark any unread budget notifications for the given budget as read,
/// except the current notification type. Call this before upserting a new
/// budget notification so stale thresholds (e.g. Budget75 when now at 90%) are cleared.
pub async fn clear_stale_budget_notifications(
    pool: &PgPool,
    budget_id: i32,
    current_type: NotificationType,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE notifications SET read = TRUE
         WHERE reference_id = $1
           AND read = FALSE
           AND notification_type IN ('budget_50','budget_75','budget_90','budget_100','budget_exceeded')
           AND notification_type != $2",
    )
    .bind(budget_id)
    .bind(current_type.as_str())
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
