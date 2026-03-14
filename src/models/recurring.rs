use std::fmt;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl Frequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Weekly => "Weekly",
            Self::Monthly => "Monthly",
            Self::Yearly => "Yearly",
        }
    }
}

impl fmt::Display for Frequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Frequency {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            "yearly" => Ok(Self::Yearly),
            _ => Err(format!("invalid frequency: {s}")),
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct RecurringTransaction {
    pub id: i32,
    pub amount: Decimal,
    pub description: String,
    pub category_id: i32,
    pub account_id: i32,
    pub transaction_type: String,
    pub payment_method: String,
    pub frequency: String,
    pub next_due: NaiveDate,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

impl RecurringTransaction {
    pub fn parsed_frequency(&self) -> Frequency {
        self.frequency.parse().unwrap_or(Frequency::Monthly)
    }

    pub fn parsed_type(&self) -> super::TransactionType {
        self.transaction_type
            .parse()
            .unwrap_or(super::TransactionType::Expense)
    }

    pub fn parsed_payment_method(&self) -> super::PaymentMethod {
        self.payment_method
            .parse()
            .unwrap_or(super::PaymentMethod::Pix)
    }
}
