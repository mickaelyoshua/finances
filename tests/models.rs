use chrono::{NaiveDate, Utc};
use rust_decimal_macros::dec;

use finances::models::*;

// -- Enum Display/FromStr roundtrips --

#[test]
fn account_type_roundtrip() {
    for variant in [AccountType::Checking, AccountType::Cash] {
        let parsed: AccountType = variant.to_string().parse().unwrap();
        assert_eq!(parsed, variant);
    }
}

#[test]
fn account_type_invalid_rejected() {
    assert!("savings".parse::<AccountType>().is_err());
}

#[test]
fn category_type_roundtrip() {
    for variant in [CategoryType::Expense, CategoryType::Income] {
        let parsed: CategoryType = variant.to_string().parse().unwrap();
        assert_eq!(parsed, variant);
    }
}

#[test]
fn category_type_invalid_rejected() {
    assert!("refund".parse::<CategoryType>().is_err());
}

#[test]
fn transaction_type_roundtrip() {
    for variant in [TransactionType::Expense, TransactionType::Income] {
        let parsed: TransactionType = variant.to_string().parse().unwrap();
        assert_eq!(parsed, variant);
    }
}

#[test]
fn transaction_type_invalid_rejected() {
    assert!("transfer".parse::<TransactionType>().is_err());
}

#[test]
fn payment_method_roundtrip() {
    for variant in [
        PaymentMethod::Pix,
        PaymentMethod::Credit,
        PaymentMethod::Debit,
        PaymentMethod::Cash,
        PaymentMethod::Boleto,
        PaymentMethod::Transfer,
    ] {
        let parsed: PaymentMethod = variant.to_string().parse().unwrap();
        assert_eq!(parsed, variant);
    }
}

#[test]
fn payment_method_invalid_rejected() {
    assert!("bitcoin".parse::<PaymentMethod>().is_err());
}

#[test]
fn budget_period_roundtrip() {
    for variant in [BudgetPeriod::Weekly, BudgetPeriod::Monthly, BudgetPeriod::Yearly] {
        let parsed: BudgetPeriod = variant.to_string().parse().unwrap();
        assert_eq!(parsed, variant);
    }
}

#[test]
fn budget_period_invalid_rejected() {
    assert!("biweekly".parse::<BudgetPeriod>().is_err());
}

#[test]
fn frequency_roundtrip() {
    for variant in [
        Frequency::Daily,
        Frequency::Weekly,
        Frequency::Monthly,
        Frequency::Yearly,
    ] {
        let parsed: Frequency = variant.to_string().parse().unwrap();
        assert_eq!(parsed, variant);
    }
}

#[test]
fn frequency_invalid_rejected() {
    assert!("hourly".parse::<Frequency>().is_err());
}

// -- TransactionType → CategoryType mapping --

#[test]
fn expense_maps_to_expense_category() {
    assert_eq!(TransactionType::Expense.category_type(), CategoryType::Expense);
}

#[test]
fn income_maps_to_income_category() {
    assert_eq!(TransactionType::Income.category_type(), CategoryType::Income);
}

// -- Account::allowed_payment_methods --

fn make_account(account_type: &str, has_credit_card: bool, has_debit_card: bool) -> Account {
    Account {
        id: 1,
        name: "Test".into(),
        account_type: account_type.into(),
        has_credit_card,
        credit_limit: if has_credit_card { Some(dec!(1000)) } else { None },
        billing_day: if has_credit_card { Some(10) } else { None },
        due_day: if has_credit_card { Some(20) } else { None },
        has_debit_card,
        active: true,
        created_at: Utc::now(),
    }
}

#[test]
fn checking_base_has_pix_boleto_transfer() {
    let acc = make_account("checking", false, false);
    let methods = acc.allowed_payment_methods();
    assert!(methods.contains(&PaymentMethod::Pix));
    assert!(methods.contains(&PaymentMethod::Boleto));
    assert!(methods.contains(&PaymentMethod::Transfer));
    assert!(!methods.contains(&PaymentMethod::Credit));
    assert!(!methods.contains(&PaymentMethod::Debit));
    assert!(!methods.contains(&PaymentMethod::Cash));
}

#[test]
fn checking_with_credit_card_includes_credit() {
    let acc = make_account("checking", true, false);
    let methods = acc.allowed_payment_methods();
    assert!(methods.contains(&PaymentMethod::Credit));
    assert!(!methods.contains(&PaymentMethod::Debit));
}

#[test]
fn checking_with_debit_card_includes_debit() {
    let acc = make_account("checking", false, true);
    let methods = acc.allowed_payment_methods();
    assert!(methods.contains(&PaymentMethod::Debit));
    assert!(!methods.contains(&PaymentMethod::Credit));
}

#[test]
fn checking_with_both_cards() {
    let acc = make_account("checking", true, true);
    let methods = acc.allowed_payment_methods();
    assert!(methods.contains(&PaymentMethod::Credit));
    assert!(methods.contains(&PaymentMethod::Debit));
}

#[test]
fn cash_account_only_cash_method() {
    let acc = make_account("cash", false, false);
    let methods = acc.allowed_payment_methods();
    assert_eq!(methods, vec![PaymentMethod::Cash]);
}

// -- Account::credit_card_is_valid --

#[test]
fn credit_card_valid_when_all_fields_set() {
    let acc = make_account("checking", true, false);
    assert!(acc.credit_card_is_valid());
}

#[test]
fn credit_card_invalid_when_limit_missing() {
    let mut acc = make_account("checking", true, false);
    acc.credit_limit = None;
    assert!(!acc.credit_card_is_valid());
}

#[test]
fn credit_card_invalid_when_billing_day_missing() {
    let mut acc = make_account("checking", true, false);
    acc.billing_day = None;
    assert!(!acc.credit_card_is_valid());
}

#[test]
fn credit_card_invalid_when_due_day_missing() {
    let mut acc = make_account("checking", true, false);
    acc.due_day = None;
    assert!(!acc.credit_card_is_valid());
}

#[test]
fn no_credit_card_always_valid() {
    let acc = make_account("checking", false, false);
    assert!(acc.credit_card_is_valid());
}

// -- BudgetPeriod::date_range --

#[test]
fn weekly_range_starts_monday() {
    // 2026-03-11 is a Wednesday
    let wed = NaiveDate::from_ymd_opt(2026, 3, 11).unwrap();
    let (start, end) = BudgetPeriod::Weekly.date_range(wed);
    assert_eq!(start, NaiveDate::from_ymd_opt(2026, 3, 9).unwrap()); // Monday
    assert_eq!(end, wed);
}

#[test]
fn weekly_range_on_monday() {
    let mon = NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
    let (start, end) = BudgetPeriod::Weekly.date_range(mon);
    assert_eq!(start, mon);
    assert_eq!(end, mon);
}

#[test]
fn monthly_range_starts_first_of_month() {
    let day = NaiveDate::from_ymd_opt(2026, 3, 13).unwrap();
    let (start, end) = BudgetPeriod::Monthly.date_range(day);
    assert_eq!(start, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap());
    assert_eq!(end, day);
}

#[test]
fn yearly_range_starts_jan_first() {
    let day = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
    let (start, end) = BudgetPeriod::Yearly.date_range(day);
    assert_eq!(start, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
    assert_eq!(end, day);
}
