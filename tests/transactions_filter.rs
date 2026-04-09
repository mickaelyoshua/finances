//! Unit tests for `cycle_option` and `TransactionFilter::to_params` —
//! verifying filter-bar state is correctly mapped to `TransactionFilterParams`.

use chrono::{NaiveDate, Utc};
use crossterm::event::KeyCode;
use rust_decimal_macros::dec;

use finances_tui::models::*;
use finances_tui::ui::i18n::Locale;
use finances_tui::ui::screens::transactions::{TransactionFilter, cycle_option};

// ── cycle_option (forward — Right key) ──────────────────────────

#[test]
fn cycle_option_none_starts_at_zero() {
    assert_eq!(cycle_option(None, 3, KeyCode::Right), Some(0));
}

#[test]
fn cycle_option_increments() {
    assert_eq!(cycle_option(Some(0), 3, KeyCode::Right), Some(1));
    assert_eq!(cycle_option(Some(1), 3, KeyCode::Right), Some(2));
}

#[test]
fn cycle_option_wraps_to_none() {
    assert_eq!(cycle_option(Some(2), 3, KeyCode::Right), None);
}

#[test]
fn cycle_option_empty_list_stays_none() {
    assert_eq!(cycle_option(None, 0, KeyCode::Right), None);
    assert_eq!(cycle_option(Some(0), 0, KeyCode::Right), None);
}

#[test]
fn cycle_option_single_element() {
    assert_eq!(cycle_option(None, 1, KeyCode::Right), Some(0));
    assert_eq!(cycle_option(Some(0), 1, KeyCode::Right), None);
}

// ── cycle_option (backward — Left key) ──────────────────────────

#[test]
fn cycle_option_left_from_none_goes_to_last() {
    assert_eq!(cycle_option(None, 3, KeyCode::Left), Some(2));
}

#[test]
fn cycle_option_left_decrements() {
    assert_eq!(cycle_option(Some(2), 3, KeyCode::Left), Some(1));
    assert_eq!(cycle_option(Some(1), 3, KeyCode::Left), Some(0));
}

#[test]
fn cycle_option_left_wraps_to_none() {
    assert_eq!(cycle_option(Some(0), 3, KeyCode::Left), None);
}

#[test]
fn cycle_option_left_empty_stays_none() {
    assert_eq!(cycle_option(None, 0, KeyCode::Left), None);
}

// ── cycle_option (Space behaves like Right) ─────────────────────

#[test]
fn cycle_option_space_goes_forward() {
    assert_eq!(cycle_option(None, 3, KeyCode::Char(' ')), Some(0));
    assert_eq!(cycle_option(Some(0), 3, KeyCode::Char(' ')), Some(1));
}

// ── TransactionFilter::to_params ─────────────────────────────────

fn make_accounts() -> Vec<Account> {
    vec![
        Account {
            id: 10,
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
            id: 20,
            name: "Cash".into(),
            account_type: "cash".into(),
            has_credit_card: false,
            credit_limit: None,
            billing_day: None,
            due_day: None,
            has_debit_card: false,
            active: true,
            created_at: Utc::now(),
        },
    ]
}

fn make_categories() -> Vec<Category> {
    vec![
        Category {
            id: 100,
            name: "Food".into(),
            name_pt: None,
            category_type: "expense".into(),
            created_at: Utc::now(),
        },
        Category {
            id: 200,
            name: "Salary".into(),
            name_pt: None,
            category_type: "income".into(),
            created_at: Utc::now(),
        },
    ]
}

#[test]
fn to_params_empty_filter_returns_defaults() {
    let filter = TransactionFilter::new(Locale::default());
    let params = filter.to_params(&make_accounts(), &make_categories());

    assert!(params.date_from.is_none());
    assert!(params.date_to.is_none());
    assert!(params.description.is_none());
    assert!(params.account_id.is_none());
    assert!(params.category_id.is_none());
    assert!(params.transaction_type.is_none());
    assert!(params.payment_method.is_none());
}

#[test]
fn to_params_valid_dates_parsed() {
    let mut filter = TransactionFilter::new(Locale::default());
    filter.date_from.value = "01-03-2026".into();
    filter.date_to.value = "31-03-2026".into();

    let params = filter.to_params(&[], &[]);

    assert_eq!(
        params.date_from,
        Some(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap())
    );
    assert_eq!(
        params.date_to,
        Some(NaiveDate::from_ymd_opt(2026, 3, 31).unwrap())
    );
}

#[test]
fn to_params_invalid_date_becomes_none() {
    let mut filter = TransactionFilter::new(Locale::default());
    filter.date_from.value = "not-a-date".into();
    filter.date_to.value = "32-13-2026".into();

    let params = filter.to_params(&[], &[]);

    assert!(params.date_from.is_none());
    assert!(params.date_to.is_none());
}

#[test]
fn to_params_description_trimmed_or_none() {
    let mut filter = TransactionFilter::new(Locale::default());
    filter.description.value = "   ".into();
    let params = filter.to_params(&[], &[]);
    assert!(params.description.is_none());

    filter.description.value = "  groceries  ".into();
    let params = filter.to_params(&[], &[]);
    assert_eq!(params.description, Some("groceries".into()));
}

#[test]
fn to_params_account_idx_maps_to_id() {
    let accounts = make_accounts();
    let mut filter = TransactionFilter::new(Locale::default());

    filter.account_idx = Some(0);
    let params = filter.to_params(&accounts, &[]);
    assert_eq!(params.account_id, Some(10));

    filter.account_idx = Some(1);
    let params = filter.to_params(&accounts, &[]);
    assert_eq!(params.account_id, Some(20));
}

#[test]
fn to_params_account_idx_out_of_bounds_becomes_none() {
    let accounts = make_accounts();
    let mut filter = TransactionFilter::new(Locale::default());
    filter.account_idx = Some(99);

    let params = filter.to_params(&accounts, &[]);
    assert!(params.account_id.is_none());
}

#[test]
fn to_params_category_idx_maps_to_id() {
    let categories = make_categories();
    let mut filter = TransactionFilter::new(Locale::default());

    filter.category_idx = Some(1);
    let params = filter.to_params(&[], &categories);
    assert_eq!(params.category_id, Some(200));
}

#[test]
fn to_params_transaction_type_idx_maps_correctly() {
    let mut filter = TransactionFilter::new(Locale::default());

    filter.transaction_type_idx = Some(0);
    let params = filter.to_params(&[], &[]);
    assert_eq!(params.transaction_type, Some(TransactionType::Expense));

    filter.transaction_type_idx = Some(1);
    let params = filter.to_params(&[], &[]);
    assert_eq!(params.transaction_type, Some(TransactionType::Income));
}

#[test]
fn to_params_payment_method_idx_maps_correctly() {
    let mut filter = TransactionFilter::new(Locale::default());

    filter.payment_method_idx = Some(0);
    let params = filter.to_params(&[], &[]);
    assert_eq!(params.payment_method, Some(PaymentMethod::Pix));

    filter.payment_method_idx = Some(1);
    let params = filter.to_params(&[], &[]);
    assert_eq!(params.payment_method, Some(PaymentMethod::Credit));

    filter.payment_method_idx = Some(5);
    let params = filter.to_params(&[], &[]);
    assert_eq!(params.payment_method, Some(PaymentMethod::Transfer));
}

#[test]
fn to_params_payment_method_out_of_bounds_becomes_none() {
    let mut filter = TransactionFilter::new(Locale::default());
    filter.payment_method_idx = Some(99);

    let params = filter.to_params(&[], &[]);
    assert!(params.payment_method.is_none());
}
