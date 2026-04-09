//! Integration tests for the DB layer. Each test gets a clean database via TRUNCATE.
//! Tests are serialized with DB_LOCK because they share a single Postgres instance.
//!
//! Uses DATABASE_URL_TEST (separate `finances_test` database) so the dev database
//! is never touched.

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sqlx::PgPool;
use tokio::sync::MutexGuard;

use finances_tui::db::{
    accounts, budgets, categories, credit_card_payments, installments, notifications, recurring,
    transactions, transfers,
};
use finances_tui::models::*;

/// Global mutex to serialize DB tests (they share one database).
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const DEFAULT_TEST_URL: &str = "postgres://finances:finances@localhost:5432/finances_test";

/// Acquire the lock, connect to the TEST database, run migrations, and truncate
/// all tables so each test starts clean. Never touches the dev database.
async fn setup() -> (MutexGuard<'static, ()>, PgPool) {
    let guard = DB_LOCK.lock().await;

    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL_TEST").unwrap_or_else(|_| DEFAULT_TEST_URL.to_string());
    let pool = finances_tui::db::create_pool(&url).await.unwrap();

    // Ensure schema is up to date
    finances_tui::db::run_migrations(&pool).await.unwrap();

    sqlx::query(
        "TRUNCATE transactions, transfers, credit_card_payments,
                  installment_purchases, budgets, recurring_transactions,
                  notifications, accounts, categories
         CASCADE",
    )
    .execute(&pool)
    .await
    .unwrap();

    (guard, pool)
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

// === Helper: create a checking account with credit card ===

async fn make_checking(pool: &PgPool, name: &str) -> Account {
    accounts::create_account(
        pool,
        &accounts::AccountParams {
            name: name.to_string(),
            account_type: AccountType::Checking,
            has_credit_card: true,
            credit_limit: Some(dec!(5000)),
            billing_day: Some(10),
            due_day: Some(20),
            has_debit_card: true,
        },
    )
    .await
    .unwrap()
}

async fn make_cash(pool: &PgPool) -> Account {
    accounts::create_account(
        pool,
        &accounts::AccountParams {
            name: "Cash".to_string(),
            account_type: AccountType::Cash,
            has_credit_card: false,
            credit_limit: None,
            billing_day: None,
            due_day: None,
            has_debit_card: false,
        },
    )
    .await
    .unwrap()
}

async fn make_txn(
    pool: &PgPool,
    amount: Decimal,
    desc: &str,
    cat_id: i32,
    acc_id: i32,
    txn_type: TransactionType,
    method: PaymentMethod,
    d: NaiveDate,
) -> Transaction {
    transactions::create_transaction(
        pool,
        &transactions::TransactionParams {
            amount,
            description: desc.to_string(),
            category_id: cat_id,
            account_id: acc_id,
            transaction_type: txn_type,
            payment_method: method,
            date: d,
        },
    )
    .await
    .unwrap()
}

async fn make_expense_category(pool: &PgPool, name: &str) -> Category {
    categories::create_category(pool, name, None, CategoryType::Expense)
        .await
        .unwrap()
}

async fn make_income_category(pool: &PgPool, name: &str) -> Category {
    categories::create_category(pool, name, None, CategoryType::Income)
        .await
        .unwrap()
}

// ═══════════════════════════════════════
// ACCOUNTS
// ═══════════════════════════════════════

#[tokio::test]
async fn create_account_returns_row() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;

    assert_eq!(acc.name, "Nubank");
    assert_eq!(acc.account_type, "checking");
    assert!(acc.has_credit_card);
    assert_eq!(acc.credit_limit, Some(dec!(5000)));
    assert_eq!(acc.billing_day, Some(10));
    assert_eq!(acc.due_day, Some(20));
    assert!(acc.has_debit_card);
    assert!(acc.active);
}

#[tokio::test]
async fn list_accounts_excludes_inactive() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    make_checking(&pool, "Inter").await;

    accounts::deactivate_account(&pool, acc.id).await.unwrap();

    let list = accounts::list_accounts(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Inter");
}

#[tokio::test]
async fn update_account_changes_fields() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;

    let updated = accounts::update_account(
        &pool,
        acc.id,
        &accounts::AccountParams {
            name: "Nu Renamed".to_string(),
            account_type: AccountType::Checking,
            has_credit_card: false,
            credit_limit: None,
            billing_day: None,
            due_day: None,
            has_debit_card: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.name, "Nu Renamed");
    assert!(!updated.has_credit_card);
    assert!(!updated.has_debit_card);
}

#[tokio::test]
async fn account_has_references_false_when_clean() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    assert!(!accounts::has_references(&pool, acc.id).await.unwrap());
}

#[tokio::test]
async fn account_has_references_true_with_transaction() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    make_txn(
        &pool,
        dec!(50),
        "Lunch",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 13),
    )
    .await;

    assert!(accounts::has_references(&pool, acc.id).await.unwrap());
}

// ═══════════════════════════════════════
// CATEGORIES
// ═══════════════════════════════════════

#[tokio::test]
async fn create_category_returns_row() {
    let (_guard, pool) = setup().await;
    let cat = make_expense_category(&pool, "Food").await;

    assert_eq!(cat.name, "Food");
    assert_eq!(cat.category_type, "expense");
}

#[tokio::test]
async fn list_categories_ordered() {
    let (_guard, pool) = setup().await;
    make_expense_category(&pool, "Zebra").await;
    make_expense_category(&pool, "Apple").await;
    make_income_category(&pool, "Salary").await;

    let list = categories::list_categories(&pool).await.unwrap();
    assert_eq!(list.len(), 3);
    // ORDER BY category_type, name → expense first, then income
    assert_eq!(list[0].name, "Apple");
    assert_eq!(list[1].name, "Zebra");
    assert_eq!(list[2].name, "Salary");
}

#[tokio::test]
async fn list_by_type_filters() {
    let (_guard, pool) = setup().await;
    make_expense_category(&pool, "Food").await;
    make_income_category(&pool, "Salary").await;

    let expenses = categories::list_by_type(&pool, CategoryType::Expense)
        .await
        .unwrap();
    assert_eq!(expenses.len(), 1);
    assert_eq!(expenses[0].name, "Food");
}

#[tokio::test]
async fn update_category_changes_fields() {
    let (_guard, pool) = setup().await;
    let cat = make_expense_category(&pool, "Food").await;

    categories::update_category(&pool, cat.id, "Groceries", None, CategoryType::Expense)
        .await
        .unwrap();

    let list = categories::list_categories(&pool).await.unwrap();
    assert_eq!(list[0].name, "Groceries");
}

#[tokio::test]
async fn create_category_with_name_pt() {
    let (_guard, pool) = setup().await;
    let cat =
        categories::create_category(&pool, "Food", Some("Alimentação"), CategoryType::Expense)
            .await
            .unwrap();

    assert_eq!(cat.name, "Food");
    assert_eq!(cat.name_pt.as_deref(), Some("Alimentação"));
}

#[tokio::test]
async fn create_category_without_name_pt() {
    let (_guard, pool) = setup().await;
    let cat = categories::create_category(&pool, "Food", None, CategoryType::Expense)
        .await
        .unwrap();

    assert_eq!(cat.name_pt, None);
}

#[tokio::test]
async fn update_category_sets_name_pt() {
    let (_guard, pool) = setup().await;
    let cat = make_expense_category(&pool, "Food").await;
    assert_eq!(cat.name_pt, None);

    categories::update_category(
        &pool,
        cat.id,
        "Food",
        Some("Alimentação"),
        CategoryType::Expense,
    )
    .await
    .unwrap();

    let list = categories::list_categories(&pool).await.unwrap();
    assert_eq!(list[0].name_pt.as_deref(), Some("Alimentação"));
}

#[tokio::test]
async fn update_category_clears_name_pt() {
    let (_guard, pool) = setup().await;
    let cat =
        categories::create_category(&pool, "Food", Some("Alimentação"), CategoryType::Expense)
            .await
            .unwrap();

    categories::update_category(&pool, cat.id, "Food", None, CategoryType::Expense)
        .await
        .unwrap();

    let list = categories::list_categories(&pool).await.unwrap();
    assert_eq!(list[0].name_pt, None);
}

#[tokio::test]
async fn delete_unreferenced_category_succeeds() {
    let (_guard, pool) = setup().await;
    let cat = make_expense_category(&pool, "Food").await;

    assert!(!categories::has_references(&pool, cat.id).await.unwrap());
    categories::delete_category(&pool, cat.id).await.unwrap();

    let list = categories::list_categories(&pool).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn category_has_references_true_with_transaction() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    make_txn(
        &pool,
        dec!(10),
        "Test",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;

    assert!(categories::has_references(&pool, cat.id).await.unwrap());
}

// ═══════════════════════════════════════
// TRANSACTIONS
// ═══════════════════════════════════════

#[tokio::test]
async fn create_transaction_returns_row() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    let txn = make_txn(
        &pool,
        dec!(99.90),
        "Supermarket",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Debit,
        date(2026, 3, 10),
    )
    .await;

    assert_eq!(txn.amount, dec!(99.90));
    assert_eq!(txn.description, "Supermarket");
    assert_eq!(txn.transaction_type, "expense");
    assert_eq!(txn.payment_method, "debit");
    assert_eq!(txn.date, date(2026, 3, 10));
    assert!(txn.installment_purchase_id.is_none());
}

#[tokio::test]
async fn list_transactions_ordered_desc() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    for day in [1, 5, 10] {
        make_txn(
            &pool,
            dec!(10),
            &format!("Day {day}"),
            cat.id,
            acc.id,
            TransactionType::Expense,
            PaymentMethod::Pix,
            date(2026, 3, day),
        )
        .await;
    }

    let filters = transactions::TransactionFilterParams::default();
    let list = transactions::list_filtered(&pool, &filters, 10, 0)
        .await
        .unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].date, date(2026, 3, 10)); // most recent first
    assert_eq!(list[2].date, date(2026, 3, 1));
}

#[tokio::test]
async fn list_by_date_range_filters() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    for day in [1, 5, 10, 20] {
        make_txn(
            &pool,
            dec!(10),
            "test",
            cat.id,
            acc.id,
            TransactionType::Expense,
            PaymentMethod::Pix,
            date(2026, 3, day),
        )
        .await;
    }

    let filters = transactions::TransactionFilterParams {
        date_from: Some(date(2026, 3, 5)),
        date_to: Some(date(2026, 3, 10)),
        ..Default::default()
    };
    let list = transactions::list_filtered(&pool, &filters, 100, 0)
        .await
        .unwrap();

    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn update_transaction_changes_fields() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    let txn = make_txn(
        &pool,
        dec!(10),
        "Old",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;

    let updated = transactions::update_transaction(
        &pool,
        txn.id,
        &transactions::TransactionParams {
            amount: dec!(25),
            description: "New".to_string(),
            category_id: cat.id,
            account_id: acc.id,
            transaction_type: TransactionType::Income,
            payment_method: PaymentMethod::Boleto,
            date: date(2026, 3, 5),
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.amount, dec!(25));
    assert_eq!(updated.description, "New");
    assert_eq!(updated.transaction_type, "income");
    assert_eq!(updated.payment_method, "boleto");
    assert_eq!(updated.date, date(2026, 3, 5));
}

#[tokio::test]
async fn delete_transaction_removes_row() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    let txn = make_txn(
        &pool,
        dec!(10),
        "test",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;

    transactions::delete_transaction(&pool, txn.id)
        .await
        .unwrap();

    let filters = transactions::TransactionFilterParams::default();
    let list = transactions::list_filtered(&pool, &filters, 100, 0)
        .await
        .unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn has_transactions_today_true() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;
    let today = chrono::Local::now().date_naive();

    // Transaction dated yesterday but created today — should still count
    make_txn(
        &pool,
        dec!(10),
        "test",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        today - chrono::Duration::days(1),
    )
    .await;

    assert!(
        transactions::has_transactions_today(&pool, today)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn has_transactions_today_false() {
    let (_guard, pool) = setup().await;
    let today = chrono::Local::now().date_naive();
    assert!(
        !transactions::has_transactions_today(&pool, today)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn has_transactions_today_installment_purchase_counts() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;
    let today = chrono::Local::now().date_naive();

    // Creating an installment purchase today counts as activity
    installments::create_installment_purchase(
        &pool,
        dec!(120),
        3,
        "Test installment",
        cat.id,
        acc.id,
        date(2026, 3, 13),
    )
    .await
    .unwrap();

    assert!(
        transactions::has_transactions_today(&pool, today)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn sum_expenses_by_category_sums_only_expenses() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;
    let income_cat = make_income_category(&pool, "Salary").await;

    // Two expenses in Food
    make_txn(
        &pool,
        dec!(30),
        "a",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;
    make_txn(
        &pool,
        dec!(20),
        "b",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 5),
    )
    .await;

    // An income in a different category (should not count)
    make_txn(
        &pool,
        dec!(1000),
        "salary",
        income_cat.id,
        acc.id,
        TransactionType::Income,
        PaymentMethod::Transfer,
        date(2026, 3, 1),
    )
    .await;

    let sum =
        transactions::sum_expenses_by_category(&pool, cat.id, date(2026, 3, 1), date(2026, 3, 31))
            .await
            .unwrap();

    assert_eq!(sum, dec!(50));
}

// ═══════════════════════════════════════
// BALANCE COMPUTATION
// ═══════════════════════════════════════

#[tokio::test]
async fn balance_income_minus_expense() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let exp_cat = make_expense_category(&pool, "Food").await;
    let inc_cat = make_income_category(&pool, "Salary").await;

    // Income R$1000 via pix
    make_txn(
        &pool,
        dec!(1000),
        "salary",
        inc_cat.id,
        acc.id,
        TransactionType::Income,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;

    // Expense R$200 via debit
    make_txn(
        &pool,
        dec!(200),
        "grocery",
        exp_cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Debit,
        date(2026, 3, 2),
    )
    .await;

    let balance = accounts::compute_balance(&pool, acc.id).await.unwrap();
    assert_eq!(balance, dec!(800));
}

#[tokio::test]
async fn balance_excludes_credit_transactions() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;
    let inc_cat = make_income_category(&pool, "Salary").await;

    // Income R$1000 via pix (counts toward checking)
    make_txn(
        &pool,
        dec!(1000),
        "salary",
        inc_cat.id,
        acc.id,
        TransactionType::Income,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;

    // Credit expense R$300 (does NOT reduce checking balance)
    make_txn(
        &pool,
        dec!(300),
        "credit purchase",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Credit,
        date(2026, 3, 2),
    )
    .await;

    let balance = accounts::compute_balance(&pool, acc.id).await.unwrap();
    assert_eq!(balance, dec!(1000)); // credit expense excluded

    let credit = accounts::compute_credit_used(&pool, acc.id).await.unwrap();
    assert_eq!(credit, dec!(300)); // shows up as credit used
}

#[tokio::test]
async fn balance_includes_transfers() {
    let (_guard, pool) = setup().await;
    let acc_a = make_checking(&pool, "Nubank").await;
    let acc_b = make_checking(&pool, "Inter").await;
    let inc_cat = make_income_category(&pool, "Salary").await;

    // Give acc_a some money
    make_txn(
        &pool,
        dec!(1000),
        "salary",
        inc_cat.id,
        acc_a.id,
        TransactionType::Income,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;

    // Transfer R$400 from A to B
    transfers::create_transfer(
        &pool,
        acc_a.id,
        acc_b.id,
        dec!(400),
        "test",
        date(2026, 3, 2),
    )
    .await
    .unwrap();

    let bal_a = accounts::compute_balance(&pool, acc_a.id).await.unwrap();
    let bal_b = accounts::compute_balance(&pool, acc_b.id).await.unwrap();

    assert_eq!(bal_a, dec!(600));
    assert_eq!(bal_b, dec!(400));
}

#[tokio::test]
async fn balance_deducts_credit_card_payments() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let inc_cat = make_income_category(&pool, "Salary").await;
    let exp_cat = make_expense_category(&pool, "Food").await;

    // Income R$1000
    make_txn(
        &pool,
        dec!(1000),
        "salary",
        inc_cat.id,
        acc.id,
        TransactionType::Income,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;

    // Credit expense R$300
    make_txn(
        &pool,
        dec!(300),
        "credit buy",
        exp_cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Credit,
        date(2026, 3, 2),
    )
    .await;

    // Pay credit card bill R$300
    credit_card_payments::create_payment(&pool, acc.id, dec!(300), date(2026, 3, 20), "pay bill")
        .await
        .unwrap();

    // Checking: 1000 (income) - 300 (cc payment) = 700
    let balance = accounts::compute_balance(&pool, acc.id).await.unwrap();
    assert_eq!(balance, dec!(700));

    // Credit used: 300 (expense) - 300 (payment) = 0
    let credit = accounts::compute_credit_used(&pool, acc.id).await.unwrap();
    assert_eq!(credit, dec!(0));
}

#[tokio::test]
async fn compute_all_balances_returns_all_active() {
    let (_guard, pool) = setup().await;
    let acc_a = make_checking(&pool, "Nubank").await;
    let acc_b = make_cash(&pool).await;
    let inc_cat = make_income_category(&pool, "Salary").await;

    make_txn(
        &pool,
        dec!(500),
        "s",
        inc_cat.id,
        acc_a.id,
        TransactionType::Income,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;

    make_txn(
        &pool,
        dec!(100),
        "cash",
        inc_cat.id,
        acc_b.id,
        TransactionType::Income,
        PaymentMethod::Cash,
        date(2026, 3, 1),
    )
    .await;

    let all = accounts::compute_all_balances(&pool).await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[&acc_a.id].0, dec!(500)); // checking balance
    assert_eq!(all[&acc_b.id].0, dec!(100));
}

// ═══════════════════════════════════════
// TRANSFERS
// ═══════════════════════════════════════

#[tokio::test]
async fn create_transfer_returns_row() {
    let (_guard, pool) = setup().await;
    let acc_a = make_checking(&pool, "Nubank").await;
    let acc_b = make_checking(&pool, "Inter").await;

    let t = transfers::create_transfer(
        &pool,
        acc_a.id,
        acc_b.id,
        dec!(100),
        "between",
        date(2026, 3, 1),
    )
    .await
    .unwrap();

    assert_eq!(t.amount, dec!(100));
    assert_eq!(t.from_account_id, acc_a.id);
    assert_eq!(t.to_account_id, acc_b.id);
}

#[tokio::test]
async fn transfer_same_account_rejected() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;

    let result =
        transfers::create_transfer(&pool, acc.id, acc.id, dec!(100), "self", date(2026, 3, 1))
            .await;

    assert!(result.is_err()); // CHECK constraint: from_account_id != to_account_id
}

// ═══════════════════════════════════════
// CREDIT CARD PAYMENTS
// ═══════════════════════════════════════

#[tokio::test]
async fn create_cc_payment_returns_row() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;

    let p = credit_card_payments::create_payment(
        &pool,
        acc.id,
        dec!(500),
        date(2026, 3, 20),
        "March bill",
    )
    .await
    .unwrap();

    assert_eq!(p.amount, dec!(500));
    assert_eq!(p.description, "March bill");
}

#[tokio::test]
async fn list_payments_in_range_filters_correctly() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;

    // Three payments: Jan, Feb, Mar
    credit_card_payments::create_payment(&pool, acc.id, dec!(100), date(2026, 1, 15), "Jan")
        .await
        .unwrap();
    credit_card_payments::create_payment(&pool, acc.id, dec!(200), date(2026, 2, 15), "Feb")
        .await
        .unwrap();
    credit_card_payments::create_payment(&pool, acc.id, dec!(300), date(2026, 3, 15), "Mar")
        .await
        .unwrap();

    // Query Feb only
    let payments = credit_card_payments::list_payments_in_range(
        &pool,
        acc.id,
        date(2026, 2, 1),
        date(2026, 2, 28),
    )
    .await
    .unwrap();

    assert_eq!(payments.len(), 1);
    assert_eq!(payments[0].amount, dec!(200));
}

// ═══════════════════════════════════════
// CREDIT CARD TRANSACTIONS
// ═══════════════════════════════════════

#[tokio::test]
async fn list_credit_by_account_filters_by_method_and_range() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    // Credit transaction in range
    make_txn(
        &pool, dec!(50), "Credit in range", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, date(2026, 2, 15),
    ).await;
    // Pix transaction in range (should be excluded)
    make_txn(
        &pool, dec!(30), "Pix in range", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Pix, date(2026, 2, 15),
    ).await;
    // Credit transaction out of range (should be excluded)
    make_txn(
        &pool, dec!(70), "Credit out of range", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, date(2026, 3, 15),
    ).await;

    let results = transactions::list_credit_by_account(
        &pool, acc.id, date(2026, 2, 1), date(2026, 2, 28),
    ).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].description, "Credit in range");
    assert_eq!(results[0].amount, dec!(50));
}

#[tokio::test]
async fn max_credit_date_returns_latest() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    make_txn(
        &pool, dec!(10), "Old", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, date(2026, 1, 1),
    ).await;
    make_txn(
        &pool, dec!(20), "New", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, date(2026, 6, 15),
    ).await;
    // Pix should be ignored
    make_txn(
        &pool, dec!(30), "Pix", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Pix, date(2026, 12, 31),
    ).await;

    let max = transactions::max_credit_date(&pool, acc.id).await.unwrap();
    assert_eq!(max, Some(date(2026, 6, 15)));
}

#[tokio::test]
async fn max_credit_date_returns_none_when_empty() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;

    let max = transactions::max_credit_date(&pool, acc.id).await.unwrap();
    assert_eq!(max, None);
}

// ═══════════════════════════════════════
// BUDGETS
// ═══════════════════════════════════════

#[tokio::test]
async fn create_budget_returns_row() {
    let (_guard, pool) = setup().await;
    let cat = make_expense_category(&pool, "Food").await;

    let b = budgets::create_budget(&pool, cat.id, dec!(800), BudgetPeriod::Monthly)
        .await
        .unwrap();

    assert_eq!(b.category_id, cat.id);
    assert_eq!(b.amount, dec!(800));
    assert_eq!(b.period, "monthly");
}

#[tokio::test]
async fn duplicate_budget_rejected() {
    let (_guard, pool) = setup().await;
    let cat = make_expense_category(&pool, "Food").await;

    budgets::create_budget(&pool, cat.id, dec!(800), BudgetPeriod::Monthly)
        .await
        .unwrap();

    // Same category + period → UNIQUE violation
    let result = budgets::create_budget(&pool, cat.id, dec!(500), BudgetPeriod::Monthly).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn update_budget_changes_amount() {
    let (_guard, pool) = setup().await;
    let cat = make_expense_category(&pool, "Food").await;

    let b = budgets::create_budget(&pool, cat.id, dec!(800), BudgetPeriod::Monthly)
        .await
        .unwrap();

    let updated = budgets::update_budget(&pool, b.id, dec!(1200))
        .await
        .unwrap();
    assert_eq!(updated.amount, dec!(1200));
}

#[tokio::test]
async fn compute_all_spending_respects_period() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    // Monthly budget
    let budget = budgets::create_budget(&pool, cat.id, dec!(500), BudgetPeriod::Monthly)
        .await
        .unwrap();

    // Expense in March
    make_txn(
        &pool,
        dec!(150),
        "groceries",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 5),
    )
    .await;

    // Expense in February (outside monthly range for March)
    make_txn(
        &pool,
        dec!(200),
        "old groceries",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 2, 15),
    )
    .await;

    let today = date(2026, 3, 13);
    let (weekly_start, _) = BudgetPeriod::Weekly.date_range(today);
    let (monthly_start, _) = BudgetPeriod::Monthly.date_range(today);
    let (yearly_start, _) = BudgetPeriod::Yearly.date_range(today);

    let spending =
        budgets::compute_all_spending(&pool, weekly_start, monthly_start, yearly_start, today)
            .await
            .unwrap();

    // Only March expense counts for this monthly budget
    assert_eq!(spending[&budget.id], dec!(150));
}

// ═══════════════════════════════════════
// INSTALLMENTS
// ═══════════════════════════════════════

#[tokio::test]
async fn create_installment_generates_n_transactions() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Electronics").await;

    let purchase = installments::create_installment_purchase(
        &pool,
        dec!(300),
        3,
        "Headphones",
        cat.id,
        acc.id,
        date(2026, 3, 10),
    )
    .await
    .unwrap();

    assert_eq!(purchase.total_amount, dec!(300));
    assert_eq!(purchase.installment_count, 3);

    let txns = installments::get_installment_transactions(&pool, purchase.id)
        .await
        .unwrap();

    assert_eq!(txns.len(), 3);
    // All are credit expenses
    for t in &txns {
        assert_eq!(t.transaction_type, "expense");
        assert_eq!(t.payment_method, "credit");
        assert_eq!(t.installment_purchase_id, Some(purchase.id));
    }
    // Dates advance monthly
    assert_eq!(txns[0].date, date(2026, 3, 10));
    assert_eq!(txns[1].date, date(2026, 4, 10));
    assert_eq!(txns[2].date, date(2026, 5, 10));
}

#[tokio::test]
async fn installment_last_absorbs_rounding() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Electronics").await;

    // R$100 / 3 = R$33.33 × 2 + R$33.34 (last absorbs remainder)
    let purchase = installments::create_installment_purchase(
        &pool,
        dec!(100),
        3,
        "Test",
        cat.id,
        acc.id,
        date(2026, 1, 1),
    )
    .await
    .unwrap();

    let txns = installments::get_installment_transactions(&pool, purchase.id)
        .await
        .unwrap();

    assert_eq!(txns[0].amount, dec!(33.33));
    assert_eq!(txns[1].amount, dec!(33.33));
    assert_eq!(txns[2].amount, dec!(33.34)); // remainder
    let total: rust_decimal::Decimal = txns.iter().map(|t| t.amount).sum();
    assert_eq!(total, dec!(100));
}

#[tokio::test]
async fn delete_installment_cascades_transactions() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Electronics").await;

    let purchase = installments::create_installment_purchase(
        &pool,
        dec!(200),
        2,
        "Keyboard",
        cat.id,
        acc.id,
        date(2026, 3, 1),
    )
    .await
    .unwrap();

    installments::delete_installment_purchase(&pool, purchase.id)
        .await
        .unwrap();

    // All generated transactions should be gone
    let filters = transactions::TransactionFilterParams::default();
    let txns = transactions::list_filtered(&pool, &filters, 100, 0)
        .await
        .unwrap();
    assert!(txns.is_empty());
}

#[tokio::test]
async fn update_installment_changes_fields_and_regenerates() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Electronics").await;
    let cat2 = make_expense_category(&pool, "Clothing").await;

    let purchase = installments::create_installment_purchase(
        &pool,
        dec!(300),
        3,
        "Headphones",
        cat.id,
        acc.id,
        date(2026, 3, 10),
    )
    .await
    .unwrap();
    assert_eq!(purchase.installment_count, 3);

    // Update: change description, amount, count, category, date
    let updated = installments::update_installment_purchase(
        &pool,
        purchase.id,
        dec!(500),
        5,
        "Jacket",
        cat2.id,
        date(2026, 4, 1),
    )
    .await
    .unwrap();

    assert_eq!(updated.id, purchase.id);
    assert_eq!(updated.description, "Jacket");
    assert_eq!(updated.total_amount, dec!(500));
    assert_eq!(updated.installment_count, 5);
    assert_eq!(updated.category_id, cat2.id);
    assert_eq!(updated.first_installment_date, date(2026, 4, 1));
    // Account unchanged
    assert_eq!(updated.account_id, acc.id);

    // Old 3 transactions should be gone, replaced by 5 new ones
    let txns = installments::get_installment_transactions(&pool, purchase.id)
        .await
        .unwrap();
    assert_eq!(txns.len(), 5);

    // All new transactions use updated fields
    for t in &txns {
        assert_eq!(t.category_id, cat2.id);
        assert_eq!(t.account_id, acc.id);
        assert_eq!(t.transaction_type, "expense");
        assert_eq!(t.payment_method, "credit");
    }

    // Descriptions follow "Jacket (n/5)" pattern
    assert_eq!(txns[0].description, "Jacket (1/5)");
    assert_eq!(txns[4].description, "Jacket (5/5)");

    // Dates advance monthly from new first date
    assert_eq!(txns[0].date, date(2026, 4, 1));
    assert_eq!(txns[1].date, date(2026, 5, 1));
    assert_eq!(txns[4].date, date(2026, 8, 1));

    // Amounts sum to new total
    let total: Decimal = txns.iter().map(|t| t.amount).sum();
    assert_eq!(total, dec!(500));
}

#[tokio::test]
async fn update_installment_preserves_account() {
    let (_guard, pool) = setup().await;
    let acc1 = make_checking(&pool, "Nubank").await;
    let acc2 = make_checking(&pool, "PicPay").await;
    let cat = make_expense_category(&pool, "Electronics").await;

    let purchase = installments::create_installment_purchase(
        &pool,
        dec!(200),
        2,
        "Mouse",
        cat.id,
        acc1.id,
        date(2026, 3, 1),
    )
    .await
    .unwrap();

    // Update does NOT take account_id — it stays as acc1
    let updated = installments::update_installment_purchase(
        &pool,
        purchase.id,
        dec!(250),
        2,
        "Mouse Pro",
        cat.id,
        date(2026, 3, 1),
    )
    .await
    .unwrap();

    assert_eq!(updated.account_id, acc1.id);

    let txns = installments::get_installment_transactions(&pool, purchase.id)
        .await
        .unwrap();
    for t in &txns {
        assert_eq!(t.account_id, acc1.id);
    }

    // acc2 just needs to exist to prove the test is meaningful
    let _ = acc2;
}

#[tokio::test]
async fn update_installment_rounding_on_count_change() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Electronics").await;

    let purchase = installments::create_installment_purchase(
        &pool,
        dec!(100),
        3,
        "Test",
        cat.id,
        acc.id,
        date(2026, 1, 1),
    )
    .await
    .unwrap();

    // Change from 3 to 7 installments — R$100/7 = R$14.29 × 6 + R$14.26
    let updated = installments::update_installment_purchase(
        &pool,
        purchase.id,
        dec!(100),
        7,
        "Test",
        cat.id,
        date(2026, 1, 1),
    )
    .await
    .unwrap();

    let txns = installments::get_installment_transactions(&pool, updated.id)
        .await
        .unwrap();

    assert_eq!(txns.len(), 7);
    // First 6 are R$14.29, last absorbs remainder
    for t in &txns[..6] {
        assert_eq!(t.amount, dec!(14.29));
    }
    assert_eq!(txns[6].amount, dec!(14.26));
    let total: Decimal = txns.iter().map(|t| t.amount).sum();
    assert_eq!(total, dec!(100));
}

// ═══════════════════════════════════════
// RECURRING
// ═══════════════════════════════════════

#[tokio::test]
async fn create_recurring_returns_row() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Subscriptions").await;

    let r = recurring::create_recurring(
        &pool,
        &recurring::RecurringParams {
            amount: dec!(29.90),
            description: "Netflix".to_string(),
            category_id: cat.id,
            account_id: acc.id,
            transaction_type: TransactionType::Expense,
            payment_method: PaymentMethod::Credit,
            frequency: Frequency::Monthly,
            next_due: date(2026, 4, 1),
        },
    )
    .await
    .unwrap();

    assert_eq!(r.amount, dec!(29.90));
    assert_eq!(r.description, "Netflix");
    assert_eq!(r.frequency, "monthly");
    assert!(r.active);
}

#[tokio::test]
async fn list_pending_filters_by_date() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Subscriptions").await;

    // Due yesterday (pending)
    recurring::create_recurring(
        &pool,
        &recurring::RecurringParams {
            amount: dec!(30),
            description: "A".to_string(),
            category_id: cat.id,
            account_id: acc.id,
            transaction_type: TransactionType::Expense,
            payment_method: PaymentMethod::Credit,
            frequency: Frequency::Monthly,
            next_due: date(2026, 3, 12),
        },
    )
    .await
    .unwrap();

    // Due tomorrow (not pending)
    recurring::create_recurring(
        &pool,
        &recurring::RecurringParams {
            amount: dec!(30),
            description: "B".to_string(),
            category_id: cat.id,
            account_id: acc.id,
            transaction_type: TransactionType::Expense,
            payment_method: PaymentMethod::Credit,
            frequency: Frequency::Monthly,
            next_due: date(2026, 3, 14),
        },
    )
    .await
    .unwrap();

    let pending = recurring::list_pending(&pool, date(2026, 3, 13))
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].description, "A");
}

#[tokio::test]
async fn update_recurring_changes_fields() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Subscriptions").await;

    let r = recurring::create_recurring(
        &pool,
        &recurring::RecurringParams {
            amount: dec!(30),
            description: "Old".to_string(),
            category_id: cat.id,
            account_id: acc.id,
            transaction_type: TransactionType::Expense,
            payment_method: PaymentMethod::Credit,
            frequency: Frequency::Monthly,
            next_due: date(2026, 4, 1),
        },
    )
    .await
    .unwrap();

    let updated = recurring::update_recurring(
        &pool,
        r.id,
        &recurring::RecurringParams {
            amount: dec!(50),
            description: "New".to_string(),
            category_id: cat.id,
            account_id: acc.id,
            transaction_type: TransactionType::Expense,
            payment_method: PaymentMethod::Pix,
            frequency: Frequency::Weekly,
            next_due: date(2026, 5, 1),
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.amount, dec!(50));
    assert_eq!(updated.description, "New");
    assert_eq!(updated.payment_method, "pix");
    assert_eq!(updated.frequency, "weekly");
    assert_eq!(updated.next_due, date(2026, 5, 1));
}

#[tokio::test]
async fn deactivate_recurring_hides_from_list() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Subscriptions").await;

    let r = recurring::create_recurring(
        &pool,
        &recurring::RecurringParams {
            amount: dec!(30),
            description: "Netflix".to_string(),
            category_id: cat.id,
            account_id: acc.id,
            transaction_type: TransactionType::Expense,
            payment_method: PaymentMethod::Credit,
            frequency: Frequency::Monthly,
            next_due: date(2026, 4, 1),
        },
    )
    .await
    .unwrap();

    recurring::deactivate_recurring(&pool, r.id).await.unwrap();

    let list = recurring::list_recurring(&pool).await.unwrap();
    assert!(list.is_empty());
}

// ═══════════════════════════════════════
// TRANSACTION FILTERS & PAGINATION
// ═══════════════════════════════════════

#[tokio::test]
async fn count_filtered_matches_list_len() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    for day in [1, 5, 10] {
        make_txn(
            &pool,
            dec!(10),
            &format!("Day {day}"),
            cat.id,
            acc.id,
            TransactionType::Expense,
            PaymentMethod::Pix,
            date(2026, 3, day),
        )
        .await;
    }

    let filters = transactions::TransactionFilterParams::default();
    let list = transactions::list_filtered(&pool, &filters, 100, 0)
        .await
        .unwrap();
    let count = transactions::count_filtered(&pool, &filters).await.unwrap();

    assert_eq!(count, list.len() as u64);
    assert_eq!(count, 3);
}

#[tokio::test]
async fn filter_by_description_ilike() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    make_txn(
        &pool,
        dec!(10),
        "Supermarket groceries",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;
    make_txn(
        &pool,
        dec!(20),
        "Restaurant dinner",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 2),
    )
    .await;

    let filters = transactions::TransactionFilterParams {
        description: Some("supermarket".into()), // case-insensitive partial match
        ..Default::default()
    };
    let list = transactions::list_filtered(&pool, &filters, 100, 0)
        .await
        .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].description, "Supermarket groceries");
}

#[tokio::test]
async fn filter_by_account_id() {
    let (_guard, pool) = setup().await;
    let acc_a = make_checking(&pool, "Nubank").await;
    let acc_b = make_checking(&pool, "Inter").await;
    let cat = make_expense_category(&pool, "Food").await;

    make_txn(
        &pool,
        dec!(10),
        "Nubank txn",
        cat.id,
        acc_a.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;
    make_txn(
        &pool,
        dec!(20),
        "Inter txn",
        cat.id,
        acc_b.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 2),
    )
    .await;

    let filters = transactions::TransactionFilterParams {
        account_id: Some(acc_b.id),
        ..Default::default()
    };
    let list = transactions::list_filtered(&pool, &filters, 100, 0)
        .await
        .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].description, "Inter txn");
}

#[tokio::test]
async fn filter_by_transaction_type() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let exp_cat = make_expense_category(&pool, "Food").await;
    let inc_cat = make_income_category(&pool, "Salary").await;

    make_txn(
        &pool,
        dec!(50),
        "expense",
        exp_cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;
    make_txn(
        &pool,
        dec!(1000),
        "income",
        inc_cat.id,
        acc.id,
        TransactionType::Income,
        PaymentMethod::Transfer,
        date(2026, 3, 1),
    )
    .await;

    let filters = transactions::TransactionFilterParams {
        transaction_type: Some(TransactionType::Income),
        ..Default::default()
    };
    let list = transactions::list_filtered(&pool, &filters, 100, 0)
        .await
        .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].description, "income");
}

#[tokio::test]
async fn filter_by_payment_method() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    make_txn(
        &pool,
        dec!(10),
        "pix txn",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;
    make_txn(
        &pool,
        dec!(20),
        "credit txn",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Credit,
        date(2026, 3, 2),
    )
    .await;

    let filters = transactions::TransactionFilterParams {
        payment_method: Some(PaymentMethod::Credit),
        ..Default::default()
    };
    let list = transactions::list_filtered(&pool, &filters, 100, 0)
        .await
        .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].description, "credit txn");
}

#[tokio::test]
async fn filter_combined_narrows_results() {
    let (_guard, pool) = setup().await;
    let acc_a = make_checking(&pool, "Nubank").await;
    let acc_b = make_checking(&pool, "Inter").await;
    let cat = make_expense_category(&pool, "Food").await;

    // Nubank, March 1
    make_txn(
        &pool,
        dec!(10),
        "Nubank Mar",
        cat.id,
        acc_a.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;
    // Nubank, March 15
    make_txn(
        &pool,
        dec!(20),
        "Nubank Mar mid",
        cat.id,
        acc_a.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 15),
    )
    .await;
    // Inter, March 1
    make_txn(
        &pool,
        dec!(30),
        "Inter Mar",
        cat.id,
        acc_b.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;

    // Filter: Nubank + date range [Mar 10, Mar 31]
    let filters = transactions::TransactionFilterParams {
        account_id: Some(acc_a.id),
        date_from: Some(date(2026, 3, 10)),
        date_to: Some(date(2026, 3, 31)),
        ..Default::default()
    };
    let list = transactions::list_filtered(&pool, &filters, 100, 0)
        .await
        .unwrap();
    let count = transactions::count_filtered(&pool, &filters).await.unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(count, 1);
    assert_eq!(list[0].description, "Nubank Mar mid");
}

#[tokio::test]
async fn pagination_offset_skips_rows() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    for day in 1..=5 {
        make_txn(
            &pool,
            dec!(10),
            &format!("txn-{day}"),
            cat.id,
            acc.id,
            TransactionType::Expense,
            PaymentMethod::Pix,
            date(2026, 3, day),
        )
        .await;
    }

    let filters = transactions::TransactionFilterParams::default();

    // Page 1: limit 2, offset 0 → 2 most recent (day 5, 4)
    let page1 = transactions::list_filtered(&pool, &filters, 2, 0)
        .await
        .unwrap();
    assert_eq!(page1.len(), 2);
    assert_eq!(page1[0].date, date(2026, 3, 5));
    assert_eq!(page1[1].date, date(2026, 3, 4));

    // Page 2: limit 2, offset 2 → next 2 (day 3, 2)
    let page2 = transactions::list_filtered(&pool, &filters, 2, 2)
        .await
        .unwrap();
    assert_eq!(page2.len(), 2);
    assert_eq!(page2[0].date, date(2026, 3, 3));
    assert_eq!(page2[1].date, date(2026, 3, 2));

    // Page 3: limit 2, offset 4 → last 1 (day 1)
    let page3 = transactions::list_filtered(&pool, &filters, 2, 4)
        .await
        .unwrap();
    assert_eq!(page3.len(), 1);
    assert_eq!(page3[0].date, date(2026, 3, 1));
}

#[tokio::test]
async fn count_filtered_with_filters() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let exp_cat = make_expense_category(&pool, "Food").await;
    let inc_cat = make_income_category(&pool, "Salary").await;

    make_txn(
        &pool,
        dec!(10),
        "a",
        exp_cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await;
    make_txn(
        &pool,
        dec!(20),
        "b",
        exp_cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 2),
    )
    .await;
    make_txn(
        &pool,
        dec!(1000),
        "c",
        inc_cat.id,
        acc.id,
        TransactionType::Income,
        PaymentMethod::Transfer,
        date(2026, 3, 3),
    )
    .await;

    let filters = transactions::TransactionFilterParams {
        transaction_type: Some(TransactionType::Expense),
        ..Default::default()
    };
    let count = transactions::count_filtered(&pool, &filters).await.unwrap();
    assert_eq!(count, 2);
}

// ═══════════════════════════════════════
// RECURRING (continued)
// ═══════════════════════════════════════

#[tokio::test]
async fn advance_next_due_updates_date() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Subscriptions").await;

    let r = recurring::create_recurring(
        &pool,
        &recurring::RecurringParams {
            amount: dec!(30),
            description: "Netflix".to_string(),
            category_id: cat.id,
            account_id: acc.id,
            transaction_type: TransactionType::Expense,
            payment_method: PaymentMethod::Credit,
            frequency: Frequency::Monthly,
            next_due: date(2026, 3, 1),
        },
    )
    .await
    .unwrap();

    let new_due = recurring::compute_next_due(r.next_due, r.parsed_frequency());
    recurring::advance_next_due(&pool, r.id, new_due)
        .await
        .unwrap();

    let list = recurring::list_recurring(&pool).await.unwrap();
    assert_eq!(list[0].next_due, date(2026, 4, 1));
}

// ═══════════════════════════════════════
// NOTIFICATIONS
// ═══════════════════════════════════════

#[tokio::test]
async fn insert_notification_and_list_unread() {
    let (_guard, pool) = setup().await;

    notifications::upsert(
        &pool,
        "You haven't logged any transactions today!",
        NotificationType::NoTransactions,
        None,
    )
    .await
    .unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].message,
        "You haven't logged any transactions today!"
    );
    assert_eq!(list[0].notification_type, "no_transactions");
    assert_eq!(list[0].reference_id, None);
    assert!(!list[0].read);
}

#[tokio::test]
async fn upsert_refreshes_existing_unread() {
    let (_guard, pool) = setup().await;

    // Insert twice with same type + reference_id
    notifications::upsert(
        &pool,
        "Budget 'Food' reached 75%",
        NotificationType::Budget75,
        Some(42),
    )
    .await
    .unwrap();

    notifications::upsert(
        &pool,
        "Budget 'Food' reached 75% — updated amount",
        NotificationType::Budget75,
        Some(42),
    )
    .await
    .unwrap();

    // Still one row, but message should be updated
    let list = notifications::list_unread(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].message,
        "Budget 'Food' reached 75% — updated amount"
    );
}

#[tokio::test]
async fn upsert_allows_reinsert_after_read() {
    let (_guard, pool) = setup().await;

    notifications::upsert(
        &pool,
        "Overdue: Netflix",
        NotificationType::OverdueRecurring,
        Some(7),
    )
    .await
    .unwrap();

    // Mark it as read
    let list = notifications::list_unread(&pool).await.unwrap();
    notifications::mark_read(&pool, list[0].id).await.unwrap();

    // Now inserting again should succeed (dedup index only covers unread)
    notifications::upsert(
        &pool,
        "Overdue: Netflix — still pending",
        NotificationType::OverdueRecurring,
        Some(7),
    )
    .await
    .unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].message, "Overdue: Netflix — still pending");
}

#[tokio::test]
async fn mark_read_single_notification() {
    let (_guard, pool) = setup().await;

    notifications::upsert(&pool, "A", NotificationType::NoTransactions, None)
        .await
        .unwrap();
    notifications::upsert(&pool, "B", NotificationType::Budget50, Some(1))
        .await
        .unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert_eq!(list.len(), 2);

    // Mark the first one as read
    let id_to_read = list.iter().find(|n| n.message == "A").unwrap().id;
    notifications::mark_read(&pool, id_to_read).await.unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].message, "B");
}

#[tokio::test]
async fn mark_all_read_clears_unread() {
    let (_guard, pool) = setup().await;

    notifications::upsert(&pool, "A", NotificationType::NoTransactions, None)
        .await
        .unwrap();
    notifications::upsert(&pool, "B", NotificationType::Budget90, Some(1))
        .await
        .unwrap();
    notifications::upsert(&pool, "C", NotificationType::OverdueRecurring, Some(2))
        .await
        .unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert_eq!(list.len(), 3);

    notifications::mark_all_read(&pool).await.unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn list_unread_excludes_read_notifications() {
    let (_guard, pool) = setup().await;

    notifications::upsert(&pool, "Read me", NotificationType::Budget100, Some(1))
        .await
        .unwrap();
    notifications::upsert(&pool, "Keep me", NotificationType::BudgetExceeded, Some(2))
        .await
        .unwrap();

    // Mark one as read
    let list = notifications::list_unread(&pool).await.unwrap();
    let read_id = list.iter().find(|n| n.message == "Read me").unwrap().id;
    notifications::mark_read(&pool, read_id).await.unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].message, "Keep me");
}

#[tokio::test]
async fn clear_stale_marks_old_budget_threshold_as_read() {
    let (_guard, pool) = setup().await;

    // Budget at 75% initially
    notifications::upsert(&pool, "at 75%", NotificationType::Budget75, Some(1))
        .await
        .unwrap();

    // Budget climbs to 90% — clear stale before upserting
    notifications::clear_stale_budget_notifications(&pool, 1, NotificationType::Budget90)
        .await
        .unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert!(list.is_empty(), "old Budget75 should be marked as read");
}

#[tokio::test]
async fn clear_stale_preserves_current_type() {
    let (_guard, pool) = setup().await;

    notifications::upsert(&pool, "at 90%", NotificationType::Budget90, Some(1))
        .await
        .unwrap();

    // Same threshold — should NOT mark itself as read
    notifications::clear_stale_budget_notifications(&pool, 1, NotificationType::Budget90)
        .await
        .unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].message, "at 90%");
}

#[tokio::test]
async fn clear_stale_ignores_non_budget_types() {
    let (_guard, pool) = setup().await;

    // OverdueRecurring with same reference_id as a budget
    notifications::upsert(
        &pool,
        "overdue",
        NotificationType::OverdueRecurring,
        Some(1),
    )
    .await
    .unwrap();

    notifications::clear_stale_budget_notifications(&pool, 1, NotificationType::Budget90)
        .await
        .unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].notification_type, "overdue_recurring");
}

#[tokio::test]
async fn clear_stale_ignores_other_budgets() {
    let (_guard, pool) = setup().await;

    // Budget75 for budget_id=99
    notifications::upsert(&pool, "other budget", NotificationType::Budget75, Some(99))
        .await
        .unwrap();

    // Clear stale for budget_id=1
    notifications::clear_stale_budget_notifications(&pool, 1, NotificationType::Budget90)
        .await
        .unwrap();

    let list = notifications::list_unread(&pool).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].message, "other budget");
}

// ═══════════════════════════════════════
// COMPUTE_ALL_BALANCES (JOIN-based)
// ═══════════════════════════════════════

#[tokio::test]
async fn compute_all_balances_credit_card_usage() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;
    let icat = make_income_category(&pool, "Salary").await;

    // Checking income
    make_txn(&pool, dec!(1000), "salary", icat.id, acc.id, TransactionType::Income, PaymentMethod::Pix, date(2026, 3, 1)).await;
    // Non-credit expense
    make_txn(&pool, dec!(200), "groceries", cat.id, acc.id, TransactionType::Expense, PaymentMethod::Pix, date(2026, 3, 2)).await;
    // Credit expense (should NOT affect checking balance but SHOULD affect credit_used)
    make_txn(&pool, dec!(300), "online shop", cat.id, acc.id, TransactionType::Expense, PaymentMethod::Credit, date(2026, 3, 3)).await;
    // Credit income/refund (reduces credit_used)
    make_txn(&pool, dec!(50), "refund", icat.id, acc.id, TransactionType::Income, PaymentMethod::Credit, date(2026, 3, 4)).await;
    // CC payment (reduces both checking and credit_used)
    credit_card_payments::create_payment(&pool, acc.id, dec!(100), date(2026, 3, 5), "cc bill").await.unwrap();

    let bals = accounts::compute_all_balances(&pool).await.unwrap();
    let (checking, credit_used) = bals[&acc.id];

    // checking = 1000 (income) - 200 (pix expense) - 100 (cc payment) = 700
    assert_eq!(checking, dec!(700));
    // credit_used = 300 (credit expense) - 50 (credit refund) - 100 (cc payment) = 150
    assert_eq!(credit_used, dec!(150));
}

#[tokio::test]
async fn compute_all_balances_with_transfers() {
    let (_guard, pool) = setup().await;
    let acc_a = make_checking(&pool, "Nubank").await;
    let acc_b = make_cash(&pool).await;
    let icat = make_income_category(&pool, "Salary").await;

    make_txn(&pool, dec!(1000), "salary", icat.id, acc_a.id, TransactionType::Income, PaymentMethod::Pix, date(2026, 3, 1)).await;
    transfers::create_transfer(&pool, acc_a.id, acc_b.id, dec!(300), "to cash", date(2026, 3, 2)).await.unwrap();

    let bals = accounts::compute_all_balances(&pool).await.unwrap();
    // acc_a: 1000 - 300 (transfer out) = 700
    assert_eq!(bals[&acc_a.id].0, dec!(700));
    // acc_b: 0 + 300 (transfer in) = 300
    assert_eq!(bals[&acc_b.id].0, dec!(300));
}

#[tokio::test]
async fn compute_all_balances_excludes_inactive() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Old").await;
    let icat = make_income_category(&pool, "Salary").await;

    make_txn(&pool, dec!(500), "s", icat.id, acc.id, TransactionType::Income, PaymentMethod::Pix, date(2026, 3, 1)).await;
    accounts::deactivate_account(&pool, acc.id).await.unwrap();

    let bals = accounts::compute_all_balances(&pool).await.unwrap();
    assert!(!bals.contains_key(&acc.id));
}

// ═══════════════════════════════════════
// BUILD STATEMENTS (period attribution)
// ═══════════════════════════════════════

#[tokio::test]
async fn build_statements_attributes_transactions_to_correct_period() {
    let (_guard, pool) = setup().await;

    // Create account with billing_day=10, due_day=20
    let acc = accounts::create_account(
        &pool,
        &accounts::AccountParams {
            name: "CC Test".to_string(),
            account_type: AccountType::Checking,
            has_credit_card: true,
            credit_limit: Some(dec!(5000)),
            billing_day: Some(10),
            due_day: Some(20),
            has_debit_card: false,
        },
    ).await.unwrap();
    let cat = make_expense_category(&pool, "Shopping").await;

    // Transaction on Jan 5 → should be in the period ending Jan 10
    make_txn(&pool, dec!(100), "jan purchase", cat.id, acc.id, TransactionType::Expense, PaymentMethod::Credit, date(2026, 1, 5)).await;
    // Transaction on Jan 15 → should be in the period ending Feb 10
    make_txn(&pool, dec!(200), "late jan purchase", cat.id, acc.id, TransactionType::Expense, PaymentMethod::Credit, date(2026, 1, 15)).await;
    // Transaction on Feb 5 → should also be in the period ending Feb 10
    make_txn(&pool, dec!(50), "feb purchase", cat.id, acc.id, TransactionType::Expense, PaymentMethod::Credit, date(2026, 2, 5)).await;

    use finances_tui::ui::screens::cc_statements::build_statements;
    let (stmts, _current_idx) = build_statements(&pool, &acc, 6).await.unwrap();

    // Find the statement with period ending on Jan 10
    let jan_stmt = stmts.iter().find(|s| {
        s.period_end.month() == 1 && s.period_end.day() == 10 && !s.is_current && !s.is_upcoming
    });
    if let Some(s) = jan_stmt {
        assert_eq!(s.total_charges, dec!(100), "Jan statement should have 100 charge");
    }

    // Find the statement with period ending on Feb 10
    let feb_stmt = stmts.iter().find(|s| {
        s.period_end.month() == 2 && s.period_end.day() == 10 && !s.is_current && !s.is_upcoming
    });
    if let Some(s) = feb_stmt {
        assert_eq!(s.total_charges, dec!(250), "Feb statement should have 200 + 50 = 250 charges");
    }
}

#[tokio::test]
async fn build_statements_payment_attribution() {
    let (_guard, pool) = setup().await;

    let acc = accounts::create_account(
        &pool,
        &accounts::AccountParams {
            name: "CC Pay Test".to_string(),
            account_type: AccountType::Checking,
            has_credit_card: true,
            credit_limit: Some(dec!(5000)),
            billing_day: Some(10),
            due_day: Some(20),
            has_debit_card: false,
        },
    ).await.unwrap();
    let cat = make_expense_category(&pool, "Shopping").await;

    // Charge in Jan period (before Jan 10)
    make_txn(&pool, dec!(500), "big purchase", cat.id, acc.id, TransactionType::Expense, PaymentMethod::Credit, date(2026, 1, 5)).await;

    // Payment on Jan 15 → after Jan 10 close, before Feb 10 close → attributed to Jan statement
    credit_card_payments::create_payment(&pool, acc.id, dec!(500), date(2026, 1, 15), "pay jan").await.unwrap();

    use finances_tui::ui::screens::cc_statements::build_statements;
    let (stmts, _) = build_statements(&pool, &acc, 6).await.unwrap();

    let jan_stmt = stmts.iter().find(|s| {
        s.period_end.month() == 1 && s.period_end.day() == 10 && !s.is_current && !s.is_upcoming
    });
    if let Some(s) = jan_stmt {
        assert_eq!(s.statement_total, dec!(500));
        assert_eq!(s.paid_amount, dec!(500));
        assert_eq!(s.balance_due(), dec!(0));
    }
}

#[tokio::test]
async fn build_statements_payment_on_close_day_is_attributed() {
    // Regression: when a statement closes today and the payment date is period_end+1
    // (tomorrow), the payment was not attributed because the attribution window
    // upper bound was `today` instead of the next closing date.
    let (_guard, pool) = setup().await;

    // billing_day=1 so that today (April 1) is exactly the latest closing date
    let acc = accounts::create_account(
        &pool,
        &accounts::AccountParams {
            name: "CC Close-Day".to_string(),
            account_type: AccountType::Checking,
            has_credit_card: true,
            credit_limit: Some(dec!(5000)),
            billing_day: Some(1),
            due_day: Some(10),
            has_debit_card: false,
        },
    ).await.unwrap();
    let cat = make_expense_category(&pool, "Shopping").await;

    // Charge in March (before April 1 close)
    make_txn(&pool, dec!(200), "march purchase", cat.id, acc.id, TransactionType::Expense, PaymentMethod::Credit, date(2026, 3, 15)).await;

    // Payment on April 2 (period_end + 1), simulating the "Pay Statement" action
    credit_card_payments::create_payment(&pool, acc.id, dec!(200), date(2026, 4, 2), "pay march stmt").await.unwrap();

    use finances_tui::ui::screens::cc_statements::build_statements;
    let (stmts, _) = build_statements(&pool, &acc, 6).await.unwrap();

    let march_stmt = stmts.iter().find(|s| {
        s.period_end.month() == 4 && s.period_end.day() == 1 && !s.is_current && !s.is_upcoming
    });
    assert!(march_stmt.is_some(), "Should find the closed statement ending April 1");
    let s = march_stmt.unwrap();
    assert_eq!(s.statement_total, dec!(200));
    assert_eq!(s.paid_amount, dec!(200), "Payment on period_end+1 should be attributed to this statement");
    assert_eq!(s.balance_due(), dec!(0));
}

// ═══════════════════════════════════════
// CONFIRM RECURRING (atomicity)
// ═══════════════════════════════════════

#[tokio::test]
async fn confirm_recurring_creates_transaction_and_advances_date() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Bills").await;

    let rec = recurring::create_recurring(
        &pool,
        &recurring::RecurringParams {
            description: "Internet".to_string(),
            amount: dec!(100),
            category_id: cat.id,
            account_id: acc.id,
            transaction_type: TransactionType::Expense,
            payment_method: PaymentMethod::Boleto,
            frequency: Frequency::Monthly,
            next_due: date(2026, 3, 1),
        },
    ).await.unwrap();

    // Simulate confirm_recurring logic atomically
    let new_next_due = recurring::compute_next_due(rec.next_due, rec.parsed_frequency());
    let mut tx = pool.begin().await.unwrap();

    sqlx::query(
        "INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(rec.amount)
    .bind(&rec.description)
    .bind(rec.category_id)
    .bind(rec.account_id)
    .bind(rec.parsed_type().as_str())
    .bind(rec.parsed_payment_method().as_str())
    .bind(rec.next_due)
    .execute(&mut *tx)
    .await
    .unwrap();

    sqlx::query("UPDATE recurring_transactions SET next_due = $2 WHERE id = $1")
        .bind(rec.id)
        .bind(new_next_due)
        .execute(&mut *tx)
        .await
        .unwrap();

    tx.commit().await.unwrap();

    // Verify transaction was created
    let filter = transactions::TransactionFilterParams {
        description: Some("Internet".to_string()),
        ..Default::default()
    };
    let txns = transactions::list_filtered(&pool, &filter, 10, 0).await.unwrap();
    assert_eq!(txns.len(), 1);
    assert_eq!(txns[0].amount, dec!(100));
    assert_eq!(txns[0].date, date(2026, 3, 1));

    // Verify next_due was advanced
    let updated = recurring::list_recurring(&pool).await.unwrap();
    let r = updated.iter().find(|r| r.id == rec.id).unwrap();
    assert_eq!(r.next_due, date(2026, 4, 1));
}

// ═══════════════════════════════════════
// BATCH SUM CREDIT
// ═══════════════════════════════════════

#[tokio::test]
async fn sum_credit_by_accounts_batch_matches_individual() {
    let (_guard, pool) = setup().await;
    let acc_a = make_checking(&pool, "Nubank").await;
    let acc_b = make_checking(&pool, "Inter").await;
    let cat = make_expense_category(&pool, "Food").await;

    make_txn(&pool, dec!(100), "a1", cat.id, acc_a.id, TransactionType::Expense, PaymentMethod::Credit, date(2026, 3, 1)).await;
    make_txn(&pool, dec!(50), "a2", cat.id, acc_a.id, TransactionType::Expense, PaymentMethod::Credit, date(2026, 3, 5)).await;
    make_txn(&pool, dec!(200), "b1", cat.id, acc_b.id, TransactionType::Expense, PaymentMethod::Credit, date(2026, 3, 3)).await;

    let from = date(2026, 3, 1);
    let to = date(2026, 3, 31);

    // Individual queries
    let (a_exp, a_inc) = transactions::sum_credit_by_account_in_range(&pool, acc_a.id, from, to).await.unwrap();
    let (b_exp, b_inc) = transactions::sum_credit_by_account_in_range(&pool, acc_b.id, from, to).await.unwrap();

    // Batch query
    let batch = transactions::sum_credit_by_accounts_batch(
        &pool,
        &[(acc_a.id, from, to), (acc_b.id, from, to)],
    ).await.unwrap();

    assert_eq!(batch[&acc_a.id], (a_exp, a_inc));
    assert_eq!(batch[&acc_b.id], (b_exp, b_inc));
}
