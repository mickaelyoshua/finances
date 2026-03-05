use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct Transfer {
    pub id: i32,
    pub from_account_id: i32,
    pub to_account_id: i32,
    pub amount: Decimal,
    pub description: String,
    pub date: NaiveDate,
    pub created_at: DateTime<Utc>,
}
