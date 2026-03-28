//! Tests for CSV export functions in `src/export.rs`.
//! Each test calls the export function, reads back the file, asserts on content, then cleans up.

use std::collections::HashMap;
use std::fs;

use chrono::{NaiveDate, Utc};
use rust_decimal_macros::dec;

use finances::export;
use finances::models::*;

// ── Helpers ─────────────────────────────────────────────────────────

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn read_csv_lines(path: &std::path::Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}

// ── export_transactions ─────────────────────────────────────────────

#[test]
fn export_transactions_headers_and_data() {
    let txns = vec![
        Transaction {
            id: 1,
            amount: dec!(150.75),
            description: "Groceries".into(),
            category_id: 10,
            account_id: 20,
            transaction_type: "expense".into(),
            payment_method: "pix".into(),
            date: date(2026, 3, 15),
            installment_purchase_id: None,
            installment_number: None,
            created_at: Utc::now(),
        },
        Transaction {
            id: 2,
            amount: dec!(3000),
            description: "Salary".into(),
            category_id: 11,
            account_id: 20,
            transaction_type: "income".into(),
            payment_method: "transfer".into(),
            date: date(2026, 3, 1),
            installment_purchase_id: None,
            installment_number: None,
            created_at: Utc::now(),
        },
    ];

    let path = export::export_transactions(
        &txns,
        |id| {
            if id == 20 {
                "Nubank".into()
            } else {
                "?".into()
            }
        },
        |id| match id {
            10 => "Food".into(),
            11 => "Salary".into(),
            _ => "?".into(),
        },
    )
    .unwrap();

    let lines = read_csv_lines(&path);
    assert_eq!(
        lines[0],
        "Date,Description,Amount,Type,Payment Method,Account,Category"
    );
    assert_eq!(
        lines[1],
        "2026-03-15,Groceries,150.75,Expense,PIX,Nubank,Food"
    );
    assert_eq!(
        lines[2],
        "2026-03-01,Salary,3000,Income,Transfer,Nubank,Salary"
    );
    assert_eq!(lines.len(), 3);

    fs::remove_file(&path).unwrap();
}

#[test]
fn export_transactions_empty_produces_header_only() {
    let path = export::export_transactions(&[], |_| "?".into(), |_| "?".into()).unwrap();

    let lines = read_csv_lines(&path);
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0],
        "Date,Description,Amount,Type,Payment Method,Account,Category"
    );

    fs::remove_file(&path).unwrap();
}

#[test]
fn export_transactions_special_chars_escaped() {
    let txns = vec![Transaction {
        id: 1,
        amount: dec!(50),
        description: "Lunch at \"Bob's\", downtown".into(),
        category_id: 10,
        account_id: 20,
        transaction_type: "expense".into(),
        payment_method: "cash".into(),
        date: date(2026, 3, 10),
        installment_purchase_id: None,
        installment_number: None,
        created_at: Utc::now(),
    }];

    let path = export::export_transactions(&txns, |_| "Cash".into(), |_| "Food".into()).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    // The csv crate should quote the description field because it contains commas and quotes
    assert!(content.contains("\"Lunch at \"\"Bob's\"\", downtown\""));

    fs::remove_file(&path).unwrap();
}

// ── export_transfers ────────────────────────────────────────────────

#[test]
fn export_transfers_headers_and_data() {
    let transfers = vec![Transfer {
        id: 1,
        from_account_id: 1,
        to_account_id: 2,
        amount: dec!(500),
        description: "Savings deposit".into(),
        date: date(2026, 3, 12),
        created_at: Utc::now(),
    }];

    let path = export::export_transfers(&transfers, |id| match id {
        1 => "Nubank".into(),
        2 => "PicPay".into(),
        _ => "?".into(),
    })
    .unwrap();

    let lines = read_csv_lines(&path);
    assert_eq!(lines[0], "Date,From Account,To Account,Amount,Description");
    assert_eq!(lines[1], "2026-03-12,Nubank,PicPay,500,Savings deposit");

    fs::remove_file(&path).unwrap();
}

// ── export_cc_payments ──────────────────────────────────────────────

#[test]
fn export_cc_payments_headers_and_data() {
    let payments = vec![CreditCardPayment {
        id: 1,
        account_id: 5,
        amount: dec!(1200.50),
        date: date(2026, 3, 20),
        description: "March bill".into(),
        created_at: Utc::now(),
    }];

    let path = export::export_cc_payments(&payments, |_| "Nubank".into()).unwrap();

    let lines = read_csv_lines(&path);
    assert_eq!(lines[0], "Date,Account,Amount,Description");
    assert_eq!(lines[1], "2026-03-20,Nubank,1200.50,March bill");

    fs::remove_file(&path).unwrap();
}

// ── export_installments ─────────────────────────────────────────────

#[test]
fn export_installments_headers_and_data() {
    let installments = vec![InstallmentPurchase {
        id: 1,
        total_amount: dec!(2400),
        installment_count: 12,
        description: "New laptop".into(),
        category_id: 10,
        account_id: 5,
        first_installment_date: date(2026, 1, 15),
        created_at: Utc::now(),
    }];

    let path =
        export::export_installments(&installments, |_| "Nubank".into(), |_| "Electronics".into())
            .unwrap();

    let lines = read_csv_lines(&path);
    assert_eq!(
        lines[0],
        "Description,Total Amount,Installments,First Date,Account,Category"
    );
    assert_eq!(lines[1], "New laptop,2400,12,2026-01-15,Nubank,Electronics");

    fs::remove_file(&path).unwrap();
}

// ── export_recurring ────────────────────────────────────────────────

#[test]
fn export_recurring_headers_and_active_flag() {
    let recurring = vec![
        RecurringTransaction {
            id: 1,
            amount: dec!(49.90),
            description: "Netflix".into(),
            category_id: 10,
            account_id: 5,
            transaction_type: "expense".into(),
            payment_method: "credit".into(),
            frequency: "monthly".into(),
            next_due: date(2026, 4, 1),
            active: true,
            created_at: Utc::now(),
        },
        RecurringTransaction {
            id: 2,
            amount: dec!(29.90),
            description: "Old gym".into(),
            category_id: 10,
            account_id: 5,
            transaction_type: "expense".into(),
            payment_method: "debit".into(),
            frequency: "monthly".into(),
            next_due: date(2026, 2, 1),
            active: false,
            created_at: Utc::now(),
        },
    ];

    let path =
        export::export_recurring(&recurring, |_| "Nubank".into(), |_| "Subscriptions".into())
            .unwrap();

    let lines = read_csv_lines(&path);
    assert_eq!(
        lines[0],
        "Description,Amount,Type,Frequency,Next Due,Account,Payment Method,Category,Active"
    );
    assert_eq!(
        lines[1],
        "Netflix,49.90,Expense,Monthly,2026-04-01,Nubank,Credit Card,Subscriptions,Yes"
    );
    assert_eq!(
        lines[2],
        "Old gym,29.90,Expense,Monthly,2026-02-01,Nubank,Debit Card,Subscriptions,No"
    );

    fs::remove_file(&path).unwrap();
}

// ── export_accounts ─────────────────────────────────────────────────

#[test]
fn export_accounts_headers_and_optional_fields() {
    let accounts = vec![
        Account {
            id: 1,
            name: "Nubank".into(),
            account_type: "checking".into(),
            has_credit_card: true,
            credit_limit: Some(dec!(5000)),
            billing_day: Some(10),
            due_day: Some(20),
            has_debit_card: true,
            active: true,
            created_at: Utc::now(),
        },
        Account {
            id: 2,
            name: "Cash wallet".into(),
            account_type: "cash".into(),
            has_credit_card: false,
            credit_limit: None,
            billing_day: None,
            due_day: None,
            has_debit_card: false,
            active: true,
            created_at: Utc::now(),
        },
    ];

    let path = export::export_accounts(&accounts).unwrap();

    let lines = read_csv_lines(&path);
    assert_eq!(
        lines[0],
        "Name,Type,Has Credit Card,Credit Limit,Billing Day,Due Day,Has Debit Card,Active"
    );
    assert_eq!(lines[1], "Nubank,Checking,Yes,5000,10,20,Yes,Yes");
    assert_eq!(lines[2], "Cash wallet,Cash,No,,,,No,Yes");

    fs::remove_file(&path).unwrap();
}

// ── export_categories ───────────────────────────────────────────────

#[test]
fn export_categories_headers_and_data() {
    let categories = vec![
        Category {
            id: 1,
            name: "Food".into(),
            name_pt: None,
            category_type: "expense".into(),
            created_at: Utc::now(),
        },
        Category {
            id: 2,
            name: "Salary".into(),
            name_pt: None,
            category_type: "income".into(),
            created_at: Utc::now(),
        },
    ];

    let path = export::export_categories(&categories).unwrap();

    let lines = read_csv_lines(&path);
    assert_eq!(lines[0], "Name,Type");
    assert_eq!(lines[1], "Food,Expense");
    assert_eq!(lines[2], "Salary,Income");

    fs::remove_file(&path).unwrap();
}

// ── export_budgets ──────────────────────────────────────────────────

#[test]
fn export_budgets_with_spent_and_percentage() {
    let budgets = vec![Budget {
        id: 1,
        category_id: 10,
        amount: dec!(500),
        period: "monthly".into(),
        created_at: Utc::now(),
    }];

    let mut spent = HashMap::new();
    spent.insert(1, dec!(350));

    let path = export::export_budgets(&budgets, |_| "Food".into(), &spent).unwrap();

    let lines = read_csv_lines(&path);
    assert_eq!(lines[0], "Category,Amount,Period,Spent,Percentage");
    assert_eq!(lines[1], "Food,500,Monthly,350,70.0%");

    fs::remove_file(&path).unwrap();
}

#[test]
fn export_budgets_zero_amount_no_panic() {
    let budgets = vec![Budget {
        id: 1,
        category_id: 10,
        amount: dec!(0),
        period: "monthly".into(),
        created_at: Utc::now(),
    }];

    let spent = HashMap::new(); // no spent entry → defaults to 0

    let path = export::export_budgets(&budgets, |_| "Food".into(), &spent).unwrap();

    let lines = read_csv_lines(&path);
    // Zero amount → 0% (no division by zero)
    assert_eq!(lines[1], "Food,0,Monthly,0,0%");

    fs::remove_file(&path).unwrap();
}
