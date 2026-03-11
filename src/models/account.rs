use std::fmt;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;

use super::PaymentMethod;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Checking,
    Cash,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Checking => "checking",
            Self::Cash => "cash",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Checking => "Checking",
            Self::Cash => "Cash",
        }
    }
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for AccountType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "checking" => Ok(Self::Checking),
            "cash" => Ok(Self::Cash),
            _ => Err(format!("invalid account type: {s}")),
        }
    }
}

/// A financial institution (bank, digital wallet) or cash wallet.
///
/// One account = one institution (e.g., "Nubank", "PicPay", "Cash").
/// Card capabilities are flags, not separate accounts — Nubank with both
/// checking and credit card is a single `Account` with `has_credit_card = true`.
#[derive(Debug, Clone, FromRow)]
pub struct Account {
    pub id: i32,
    pub name: String,
    pub account_type: String,
    pub has_credit_card: bool,
    pub credit_limit: Option<Decimal>,
    pub billing_day: Option<i16>,
    pub due_day: Option<i16>,
    pub has_debit_card: bool,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

impl Account {
    pub fn parsed_type(&self) -> AccountType {
        self.account_type.parse().unwrap_or(AccountType::Checking)
    }

    /// Returns all payment methods this account supports based on its type and capabilities.
    pub fn allowed_payment_methods(&self) -> Vec<PaymentMethod> {
        let mut methods = Vec::new();
        match self.parsed_type() {
            AccountType::Checking => {
                methods.push(PaymentMethod::Pix);
                methods.push(PaymentMethod::Boleto);
                methods.push(PaymentMethod::Transfer);
                if self.has_credit_card {
                    methods.push(PaymentMethod::Credit);
                }
                if self.has_debit_card {
                    methods.push(PaymentMethod::Debit);
                }
            }
            AccountType::Cash => {
                methods.push(PaymentMethod::Cash);
            }
        }
        methods
    }

    /// Validates that credit card fields are consistent.
    /// Returns true if has_credit_card is false, or all required fields are set.
    pub fn credit_card_is_valid(&self) -> bool {
        if self.has_credit_card {
            self.credit_limit.is_some() && self.billing_day.is_some() && self.due_day.is_some()
        } else {
            true
        }
    }
}
