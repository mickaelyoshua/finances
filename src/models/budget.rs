use std::fmt;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetPeriod {
    Weekly,
    Monthly,
    Yearly,
}

impl BudgetPeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }
}

impl fmt::Display for BudgetPeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for BudgetPeriod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            "yearly" => Ok(Self::Yearly),
            _ => Err(format!("invalid budget period: {s}")),
        }
    }
}

#[derive(Debug, FromRow)]
pub struct Budget {
    pub id: i32,
    pub category_id: i32,
    pub amount: Decimal,
    pub period: String,
    pub created_at: DateTime<Utc>,
}

impl Budget {
    pub fn parsed_period(&self) -> BudgetPeriod {
        self.period.parse().expect("invalid period in database")
    }
}
