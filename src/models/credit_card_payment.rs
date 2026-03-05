use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct CreditCardPayment {
    pub id: i32,
    pub account_id: i32,
    pub amount: Decimal,
    pub date: NaiveDate,
    pub description: String,
    pub created_at: DateTime<Utc>,
}
