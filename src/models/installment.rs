use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct InstallmentPurchase {
    pub id: i32,
    pub total_amount: Decimal,
    pub installment_count: i16,
    pub description: String,
    pub category_id: i32,
    pub account_id: i32,
    pub first_installment_date: NaiveDate,
    pub created_at: DateTime<Utc>,
}
