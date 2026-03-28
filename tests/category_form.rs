//! Unit tests for `CategoryForm` — validation of the name_pt field,
//! edit mode prefilling, and localized labels.

use chrono::Utc;

use finances::models::*;
use finances::ui::i18n::Locale;
use finances::ui::screens::categories::{CategoryForm, CategoryFormMode};

fn make_category(name_pt: Option<&str>) -> Category {
    Category {
        id: 1,
        name: "Food".into(),
        name_pt: name_pt.map(|s| s.to_string()),
        category_type: "expense".into(),
        created_at: Utc::now(),
    }
}

// ── Validation ──────────────────────────────────────────────────

#[test]
fn validate_with_name_and_name_pt() {
    let mut form = CategoryForm::new_create(Locale::En);
    form.name.value = "Food".into();
    form.name_pt.value = "Alimentação".into();

    let result = form.validate(Locale::En).unwrap();
    assert_eq!(result.name, "Food");
    assert_eq!(result.name_pt, Some("Alimentação".into()));
    assert_eq!(result.category_type, CategoryType::Expense);
}

#[test]
fn validate_without_name_pt_returns_none() {
    let mut form = CategoryForm::new_create(Locale::En);
    form.name.value = "Food".into();
    form.name_pt.value = "".into();

    let result = form.validate(Locale::En).unwrap();
    assert_eq!(result.name_pt, None);
}

#[test]
fn validate_whitespace_name_pt_is_none() {
    let mut form = CategoryForm::new_create(Locale::En);
    form.name.value = "Food".into();
    form.name_pt.value = "   ".into();

    let result = form.validate(Locale::En).unwrap();
    assert_eq!(result.name_pt, None);
}

#[test]
fn validate_empty_name_rejected() {
    let mut form = CategoryForm::new_create(Locale::En);
    form.name.value = "".into();

    let result = form.validate(Locale::En);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("required"));
}

#[test]
fn validate_whitespace_name_rejected() {
    let mut form = CategoryForm::new_create(Locale::En);
    form.name.value = "   ".into();

    let result = form.validate(Locale::En);
    assert!(result.is_err());
}

#[test]
fn validate_error_message_localized_pt() {
    let mut form = CategoryForm::new_create(Locale::Pt);
    form.name.value = "".into();

    let result = form.validate(Locale::Pt);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("obrigatório"));
}

// ── new_edit prefilling ─────────────────────────────────────────

#[test]
fn new_edit_prefills_name_pt() {
    let cat = make_category(Some("Alimentação"));
    let form = CategoryForm::new_edit(&cat, Locale::En);

    assert_eq!(form.name.value, "Food");
    assert_eq!(form.name_pt.value, "Alimentação");
    assert!(matches!(form.mode, CategoryFormMode::Edit(1)));
}

#[test]
fn new_edit_no_name_pt_leaves_empty() {
    let cat = make_category(None);
    let form = CategoryForm::new_edit(&cat, Locale::En);

    assert_eq!(form.name.value, "Food");
    assert_eq!(form.name_pt.value, "");
}

// ── Form labels localized ───────────────────────────────────────

#[test]
fn new_create_labels_in_english() {
    let form = CategoryForm::new_create(Locale::En);
    assert_eq!(form.name.label, "Name");
    assert_eq!(form.name_pt.label, "Portuguese Name");
}

#[test]
fn new_create_labels_in_portuguese() {
    let form = CategoryForm::new_create(Locale::Pt);
    assert_eq!(form.name.label, "Nome");
    assert_eq!(form.name_pt.label, "Nome em Português");
}
