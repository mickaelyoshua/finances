use std::fmt;

use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    NoTransactions,
    OverdueRecurring,
    Budget50,
    Budget75,
    Budget90,
    Budget100,
    BudgetExceeded,
}

impl NotificationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoTransactions => "no_transactions",
            Self::OverdueRecurring => "overdue_recurring",
            Self::Budget50 => "budget_50",
            Self::Budget75 => "budget_75",
            Self::Budget90 => "budget_90",
            Self::Budget100 => "budget_100",
            Self::BudgetExceeded => "budget_exceeded",
        }
    }
}

impl fmt::Display for NotificationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for NotificationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "no_transactions" => Ok(Self::NoTransactions),
            "overdue_recurring" => Ok(Self::OverdueRecurring),
            "budget_50" => Ok(Self::Budget50),
            "budget_75" => Ok(Self::Budget75),
            "budget_90" => Ok(Self::Budget90),
            "budget_100" => Ok(Self::Budget100),
            "budget_exceeded" => Ok(Self::BudgetExceeded),
            _ => Err(format!("invalid notification type: {s}")),
        }
    }
}

/// A persistent notification stored in the DB by `--notify` and displayed in the TUI dashboard.
///
/// `reference_id` points to the source row that triggered the notification:
/// - `None` for `NoTransactions` (no specific source)
/// - `recurring_transactions.id` for `OverdueRecurring`
/// - `budgets.id` for all `Budget*` types
///
/// No FK constraint — it's used only for dedup, not referential integrity.
#[derive(Debug, Clone, FromRow)]
pub struct Notification {
    pub id: i32,
    pub message: String,
    pub notification_type: String,
    pub reference_id: Option<i32>,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

impl Notification {
    pub fn parsed_type(&self) -> NotificationType {
        self.notification_type
            .parse()
            .unwrap_or(NotificationType::NoTransactions)
    }
}
