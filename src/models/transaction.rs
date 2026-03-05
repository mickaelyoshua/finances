use std::fmt;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Expense,
    Income,
}

impl TransactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Expense => "expense",
            Self::Income => "income",
        }
    }
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for TransactionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "expense" => Ok(Self::Expense),
            "income" => Ok(Self::Income),
            _ => Err(format!("invalid transaction type: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentMethod {
    Pix,
    Credit,
    Debit,
    Cash,
    Boleto,
    Transfer,
}

impl PaymentMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pix => "pix",
            Self::Credit => "credit",
            Self::Debit => "debit",
            Self::Cash => "cash",
            Self::Boleto => "boleto",
            Self::Transfer => "transfer",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Pix => "PIX",
            Self::Credit => "Credit Card",
            Self::Debit => "Debit Card",
            Self::Cash => "Cash",
            Self::Boleto => "Boleto",
            Self::Transfer => "Transfer",
        }
    }
}

impl fmt::Display for PaymentMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for PaymentMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pix" => Ok(Self::Pix),
            "credit" => Ok(Self::Credit),
            "debit" => Ok(Self::Debit),
            "cash" => Ok(Self::Cash),
            "boleto" => Ok(Self::Boleto),
            "transfer" => Ok(Self::Transfer),
            _ => Err(format!("invalid payment method: {s}")),
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct Transaction {
    pub id: i32,
    pub amount: Decimal,
    pub description: String,
    pub category_id: i32,
    pub account_id: i32,
    pub transaction_type: String,
    pub payment_method: String,
    pub date: NaiveDate,
    pub installment_purchase_id: Option<i32>,
    pub installment_number: Option<i16>,
    pub created_at: DateTime<Utc>,
}
