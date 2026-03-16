//! Unit tests for `TransactionForm::validate` — date parsing, amount conversion,
//! category filtering by transaction type, and boundary/error cases.

use chrono::Utc;
use rust_decimal_macros::dec;

use finances::models::*;
use finances::ui::screens::transactions::TransactionForm;

fn make_accounts() -> Vec<Account> {
    vec![Account {
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
    }]
}

fn make_categories() -> Vec<Category> {
    vec![
        Category {
            id: 100,
            name: "Food".into(),
            category_type: "expense".into(),
            created_at: Utc::now(),
        },
        Category {
            id: 200,
            name: "Salary".into(),
            category_type: "income".into(),
            created_at: Utc::now(),
        },
    ]
}

fn valid_form() -> TransactionForm {
    let mut form = TransactionForm::new_create();
    form.date.value = "15-03-2026".into();
    form.description.value = "Supermarket".into();
    form.amount.value = "99,90".into();
    form.transaction_type = TransactionType::Expense;
    form.account_idx = 0;
    form.payment_method_idx = 0; // Pix (first for checking)
    form.category_idx = 0; // Food (first expense category)
    form
}

// ── Happy path ───────────────────────────────────────────────────

#[test]
fn validate_valid_form_succeeds() {
    let form = valid_form();
    let accounts = make_accounts();
    let categories = make_categories();

    let result = form.validate(&accounts, &categories);
    assert!(result.is_ok());

    let v = result.unwrap();
    assert_eq!(v.amount, dec!(99.90));
    assert_eq!(v.description, "Supermarket");
    assert_eq!(v.account_id, 10);
    assert_eq!(v.category_id, 100);
    assert_eq!(v.transaction_type, TransactionType::Expense);
    assert_eq!(v.payment_method, PaymentMethod::Pix);
}

// ── Validation errors ────────────────────────────────────────────

#[test]
fn validate_bad_date_rejected() {
    let mut form = valid_form();
    form.date.value = "not-a-date".into();

    let result = form.validate(&make_accounts(), &make_categories());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("date"));
}

#[test]
fn validate_empty_description_rejected() {
    let mut form = valid_form();
    form.description.value = "   ".into();

    let result = form.validate(&make_accounts(), &make_categories());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Description"));
}

#[test]
fn validate_zero_amount_rejected() {
    let mut form = valid_form();
    form.amount.value = "0".into();

    let result = form.validate(&make_accounts(), &make_categories());
    assert!(result.is_err());
}

#[test]
fn validate_negative_amount_rejected() {
    let mut form = valid_form();
    form.amount.value = "-50".into();

    let result = form.validate(&make_accounts(), &make_categories());
    assert!(result.is_err());
}

#[test]
fn validate_garbage_amount_rejected() {
    let mut form = valid_form();
    form.amount.value = "abc".into();

    let result = form.validate(&make_accounts(), &make_categories());
    assert!(result.is_err());
}

#[test]
fn validate_no_accounts_rejected() {
    let form = valid_form();

    let result = form.validate(&[], &make_categories());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("account"));
}

#[test]
fn validate_no_matching_category_rejected() {
    let mut form = valid_form();
    form.transaction_type = TransactionType::Income;
    form.category_idx = 99; // out of bounds for income categories

    let result = form.validate(&make_accounts(), &make_categories());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("category"));
}

#[test]
fn validate_comma_amount_accepted() {
    let mut form = valid_form();
    form.amount.value = "1.234,56".into();

    // parse_positive_amount replaces comma with dot, but "1.234.56" is invalid
    // The user types "1234,56" (no thousands separator in input)
    form.amount.value = "1234,56".into();

    let result = form.validate(&make_accounts(), &make_categories());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().amount, dec!(1234.56));
}

#[test]
fn validate_income_uses_income_categories() {
    let mut form = valid_form();
    form.transaction_type = TransactionType::Income;
    form.category_idx = 0; // first income category = Salary (id: 200)

    let result = form.validate(&make_accounts(), &make_categories());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().category_id, 200);
}

#[test]
fn validate_payment_method_out_of_bounds_rejected() {
    let mut form = valid_form();
    form.payment_method_idx = 99;

    let result = form.validate(&make_accounts(), &make_categories());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("payment method"));
}
