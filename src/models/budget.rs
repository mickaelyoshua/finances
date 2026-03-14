use std::fmt;

use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc};
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

    pub fn label(&self) -> &'static str {
        match self {
            Self::Weekly => "Weekly",
            Self::Monthly => "Monthly",
            Self::Yearly => "Yearly",
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

#[derive(Debug, Clone, FromRow)]
pub struct Budget {
    pub id: i32,
    pub category_id: i32,
    pub amount: Decimal,
    pub period: String,
    pub created_at: DateTime<Utc>,
}

impl BudgetPeriod {
    /// Returns the (start, end) date range for this budget period containing `today`.
    pub fn date_range(self, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        match self {
            Self::Weekly => {
                let days_since_monday = today.weekday().num_days_from_monday();
                let start = today - TimeDelta::days(days_since_monday as i64);
                (start, today)
            }
            Self::Monthly => {
                let start = today.with_day(1).unwrap();
                (start, today)
            }
            Self::Yearly => {
                let start = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
                (start, today)
            }
        }
    }
}

impl Budget {
    pub fn parsed_period(&self) -> BudgetPeriod {
        self.period.parse().unwrap_or(BudgetPeriod::Monthly)
    }
}
