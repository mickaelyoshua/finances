//! Unit tests for `InstallmentForm` — new_edit constructor, validation in edit mode,
//! and correct account/category index mapping.

use chrono::Utc;
use rust_decimal_macros::dec;

use finances::models::*;
use finances::ui::i18n::Locale;
use finances::ui::screens::installments::{InstallmentForm, InstallmentFormMode};

fn make_accounts() -> Vec<Account> {
    vec![
        Account {
            id: 1,
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
        Account {
            id: 2,
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
            id: 3,
            name: "PicPay".into(),
            account_type: "checking".into(),
            has_credit_card: true,
            credit_limit: Some(dec!(3000)),
            billing_day: Some(15),
            due_day: Some(25),
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
        Category {
            id: 300,
            name: "Transport".into(),
            name_pt: None,
            category_type: "expense".into(),
            created_at: Utc::now(),
        },
    ]
}

fn make_purchase() -> InstallmentPurchase {
    InstallmentPurchase {
        id: 42,
        total_amount: dec!(600),
        installment_count: 6,
        description: "Laptop Stand".into(),
        category_id: 300, // Transport (second expense category, index 1)
        account_id: 3,    // PicPay (second credit account, index 1)
        first_installment_date: chrono::NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
        created_at: Utc::now(),
    }
}

#[test]
fn new_edit_prefills_all_fields() {
    let accounts = make_accounts();
    let categories = make_categories();
    let ip = make_purchase();

    let form = InstallmentForm::new_edit(&ip, &accounts, &categories, Locale::default());

    assert_eq!(form.description.value, "Laptop Stand");
    assert_eq!(form.total_amount.value, "600");
    assert_eq!(form.installment_count.value, "6");
    assert_eq!(form.first_date.value, "15-03-2026");
    assert!(matches!(form.mode, InstallmentFormMode::Edit(42)));
    assert_eq!(form.active_field, 0);
    assert!(form.error.is_none());
    assert!(form.confirmation.is_none());
}

#[test]
fn new_edit_finds_correct_account_idx() {
    let accounts = make_accounts();
    let categories = make_categories();
    let ip = make_purchase(); // account_id=3 (PicPay)

    let form = InstallmentForm::new_edit(&ip, &accounts, &categories, Locale::default());

    // Credit accounts: [Nubank(id=2), PicPay(id=3)] → PicPay is index 1
    assert_eq!(form.account_idx, 1);
}

#[test]
fn new_edit_finds_correct_category_idx() {
    let accounts = make_accounts();
    let categories = make_categories();
    let ip = make_purchase(); // category_id=300 (Transport)

    let form = InstallmentForm::new_edit(&ip, &accounts, &categories, Locale::default());

    // Expense categories: [Food(id=100), Transport(id=300)] → Transport is index 1
    assert_eq!(form.category_idx, 1);
}

#[test]
fn new_edit_falls_back_to_zero_for_missing_account() {
    let accounts = make_accounts();
    let categories = make_categories();
    let mut ip = make_purchase();
    ip.account_id = 999; // nonexistent

    let form = InstallmentForm::new_edit(&ip, &accounts, &categories, Locale::default());
    assert_eq!(form.account_idx, 0);
}

#[test]
fn new_edit_falls_back_to_zero_for_missing_category() {
    let accounts = make_accounts();
    let categories = make_categories();
    let mut ip = make_purchase();
    ip.category_id = 999; // nonexistent

    let form = InstallmentForm::new_edit(&ip, &accounts, &categories, Locale::default());
    assert_eq!(form.category_idx, 0);
}

#[test]
fn validate_edit_form_succeeds() {
    let accounts = make_accounts();
    let categories = make_categories();
    let ip = make_purchase();

    let form = InstallmentForm::new_edit(&ip, &accounts, &categories, Locale::default());
    let result = form.validate(&accounts, &categories, Locale::default());

    assert!(result.is_ok());
    let v = result.unwrap();
    assert_eq!(v.description, "Laptop Stand");
    assert_eq!(v.total_amount, dec!(600));
    assert_eq!(v.installment_count, 6);
    assert_eq!(v.account_id, 3); // PicPay
    assert_eq!(v.category_id, 300); // Transport
}

#[test]
fn validate_edit_rejects_empty_description() {
    let accounts = make_accounts();
    let categories = make_categories();
    let ip = make_purchase();

    let mut form = InstallmentForm::new_edit(&ip, &accounts, &categories, Locale::default());
    form.description.value = "   ".into();

    let result = form.validate(&accounts, &categories, Locale::default());
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Description is required");
}

#[test]
fn validate_edit_rejects_single_installment() {
    let accounts = make_accounts();
    let categories = make_categories();
    let ip = make_purchase();

    let mut form = InstallmentForm::new_edit(&ip, &accounts, &categories, Locale::default());
    form.installment_count.value = "1".into();

    let result = form.validate(&accounts, &categories, Locale::default());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("at least 2"));
}
