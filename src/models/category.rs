use std::fmt;

use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryType {
    Expense,
    Income,
}

impl CategoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Expense => "expense",
            Self::Income => "income",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Expense => "Expense",
            Self::Income => "Income",
        }
    }
}

impl fmt::Display for CategoryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for CategoryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "expense" => Ok(Self::Expense),
            "income" => Ok(Self::Income),
            _ => Err(format!("invalid category type: {s}")),
        }
    }
}

#[derive(Debug, FromRow)]
pub struct Category {
    pub id: i32,
    pub name: String,
    pub name_pt: Option<String>,
    pub category_type: String,
    pub created_at: DateTime<Utc>,
}

impl Category {
    pub fn parsed_type(&self) -> CategoryType {
        self.category_type.parse().unwrap_or(CategoryType::Expense)
    }
}
