//! CSV export for all entity types.
//!
//! Files are written to `~/.local/share/finances/exports/` with timestamped names.
//! Amounts are raw decimals (e.g., `1234.56`), not BRL-formatted, so the output
//! is directly importable into spreadsheets without locale-aware parsing.
//!
//! Export functions take closures for ID→name resolution (`account_id → "Nubank"`)
//! because models store foreign-key IDs, not denormalized names.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, Result};
use chrono::Local;
use rust_decimal::Decimal;
use tracing::info;

use crate::models::{
    Account, Budget, Category, CreditCardPayment, InstallmentPurchase, RecurringTransaction,
    Transaction, Transfer,
};

/// Monotonic counter to guarantee unique filenames even when called within the same millisecond.
static EXPORT_SEQ: AtomicU32 = AtomicU32::new(0);

fn export_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .context("could not resolve data directory")?
        .join("finances")
        .join("exports");
    std::fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

fn export_path(name: &str) -> Result<PathBuf> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let seq = EXPORT_SEQ.fetch_add(1, Ordering::Relaxed);
    Ok(export_dir()?.join(format!("{name}_{timestamp}_{seq}.csv")))
}

pub fn export_transactions(
    txns: &[Transaction],
    account_name: impl Fn(i32) -> String,
    category_name: impl Fn(i32) -> String,
) -> Result<PathBuf> {
    let path = export_path("transactions")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record([
        "Date",
        "Description",
        "Amount",
        "Type",
        "Payment Method",
        "Account",
        "Category",
    ])?;
    for t in txns {
        wtr.write_record([
            t.date.to_string(),
            t.description.clone(),
            t.amount.to_string(),
            t.parsed_type().label().to_string(),
            t.parsed_payment_method().label().to_string(),
            account_name(t.account_id),
            category_name(t.category_id),
        ])?;
    }
    wtr.flush()?;
    info!(rows = txns.len(), path = %path.display(), "exported transactions");
    Ok(path)
}

pub fn export_transfers(
    transfers: &[Transfer],
    account_name: impl Fn(i32) -> String,
) -> Result<PathBuf> {
    let path = export_path("transfers")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record([
        "Date",
        "From Account",
        "To Account",
        "Amount",
        "Description",
    ])?;
    for t in transfers {
        wtr.write_record([
            t.date.to_string(),
            account_name(t.from_account_id),
            account_name(t.to_account_id),
            t.amount.to_string(),
            t.description.clone(),
        ])?;
    }
    wtr.flush()?;
    info!(rows = transfers.len(), path = %path.display(), "exported transfers");
    Ok(path)
}

pub fn export_cc_payments(
    payments: &[CreditCardPayment],
    account_name: impl Fn(i32) -> String,
) -> Result<PathBuf> {
    let path = export_path("cc_payments")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record(["Date", "Account", "Amount", "Description"])?;
    for p in payments {
        wtr.write_record([
            p.date.to_string(),
            account_name(p.account_id),
            p.amount.to_string(),
            p.description.clone(),
        ])?;
    }
    wtr.flush()?;
    info!(rows = payments.len(), path = %path.display(), "exported cc_payments");
    Ok(path)
}

pub fn export_installments(
    installments: &[InstallmentPurchase],
    account_name: impl Fn(i32) -> String,
    category_name: impl Fn(i32) -> String,
) -> Result<PathBuf> {
    let path = export_path("installments")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record([
        "Description",
        "Total Amount",
        "Installments",
        "First Date",
        "Account",
        "Category",
    ])?;
    for i in installments {
        wtr.write_record([
            i.description.clone(),
            i.total_amount.to_string(),
            i.installment_count.to_string(),
            i.first_installment_date.to_string(),
            account_name(i.account_id),
            category_name(i.category_id),
        ])?;
    }
    wtr.flush()?;
    info!(rows = installments.len(), path = %path.display(), "exported installments");
    Ok(path)
}

pub fn export_recurring(
    recurring: &[RecurringTransaction],
    account_name: impl Fn(i32) -> String,
    category_name: impl Fn(i32) -> String,
) -> Result<PathBuf> {
    let path = export_path("recurring")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record([
        "Description",
        "Amount",
        "Type",
        "Frequency",
        "Next Due",
        "Account",
        "Payment Method",
        "Category",
        "Active",
    ])?;
    for r in recurring {
        wtr.write_record([
            r.description.clone(),
            r.amount.to_string(),
            r.parsed_type().label().to_string(),
            r.parsed_frequency().label().to_string(),
            r.next_due.to_string(),
            account_name(r.account_id),
            r.parsed_payment_method().label().to_string(),
            category_name(r.category_id),
            if r.active { "Yes" } else { "No" }.to_string(),
        ])?;
    }
    wtr.flush()?;
    info!(rows = recurring.len(), path = %path.display(), "exported recurring");
    Ok(path)
}

pub fn export_accounts(accounts: &[Account]) -> Result<PathBuf> {
    let path = export_path("accounts")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record([
        "Name",
        "Type",
        "Has Credit Card",
        "Credit Limit",
        "Billing Day",
        "Due Day",
        "Has Debit Card",
        "Active",
    ])?;
    for a in accounts {
        wtr.write_record([
            a.name.clone(),
            a.parsed_type().label().to_string(),
            if a.has_credit_card { "Yes" } else { "No" }.to_string(),
            a.credit_limit.map_or(String::new(), |d| d.to_string()),
            a.billing_day.map_or(String::new(), |d| d.to_string()),
            a.due_day.map_or(String::new(), |d| d.to_string()),
            if a.has_debit_card { "Yes" } else { "No" }.to_string(),
            if a.active { "Yes" } else { "No" }.to_string(),
        ])?;
    }
    wtr.flush()?;
    info!(rows = accounts.len(), path = %path.display(), "exported accounts");
    Ok(path)
}

pub fn export_categories(categories: &[Category]) -> Result<PathBuf> {
    let path = export_path("categories")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record(["Name", "Type"])?;
    for c in categories {
        wtr.write_record([c.name.clone(), c.parsed_type().label().to_string()])?;
    }
    wtr.flush()?;
    info!(rows = categories.len(), path = %path.display(), "exported categories");
    Ok(path)
}

pub fn export_budgets(
    budgets: &[Budget],
    category_name: impl Fn(i32) -> String,
    budget_spent: &HashMap<i32, Decimal>,
) -> Result<PathBuf> {
    let path = export_path("budgets")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record(["Category", "Amount", "Period", "Spent", "Percentage"])?;
    for b in budgets {
        let spent = budget_spent.get(&b.id).copied().unwrap_or(Decimal::ZERO);
        let pct = if b.amount > Decimal::ZERO {
            (spent / b.amount * Decimal::from(100)).round_dp(1)
        } else {
            Decimal::ZERO
        };
        wtr.write_record([
            category_name(b.category_id),
            b.amount.to_string(),
            b.parsed_period().label().to_string(),
            spent.to_string(),
            format!("{pct}%"),
        ])?;
    }
    wtr.flush()?;
    info!(rows = budgets.len(), path = %path.display(), "exported budgets");
    Ok(path)
}
