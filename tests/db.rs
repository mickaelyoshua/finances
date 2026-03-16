use chrono::NaiveDate;
use rust_decimal_macros::dec;
use sqlx::PgPool;
use tokio::sync::MutexGuard;

use finances::db::{
    accounts, budgets, categories, credit_card_payments, installments, recurring, transactions,
    transfers,
};
use finances::models::*;

/// Global mutex to serialize DB tests (they share one database).
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Acquire the lock, connect, and truncate all tables so each test starts clean.
async fn setup() -> (MutexGuard<'static, ()>, PgPool) {
    let guard = DB_LOCK.lock().await;

    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = finances::db::create_pool(&url).await.unwrap();

    sqlx::query(
        "TRUNCATE transactions, transfers, credit_card_payments,
                  installment_purchases, budgets, recurring_transactions,
                  accounts, categories
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
        name,
        AccountType::Checking,
        true,
        Some(dec!(5000)),
        Some(10),
        Some(20),
        true,
    )
    .await
    .unwrap()
}

async fn make_cash(pool: &PgPool) -> Account {
    accounts::create_account(pool, "Cash", AccountType::Cash, false, None, None, None, false)
        .await
        .unwrap()
}

async fn make_expense_category(pool: &PgPool, name: &str) -> Category {
    categories::create_category(pool, name, CategoryType::Expense)
        .await
        .unwrap()
}

async fn make_income_category(pool: &PgPool, name: &str) -> Category {
    categories::create_category(pool, name, CategoryType::Income)
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
        "Nu Renamed",
        AccountType::Checking,
        false,
        None,
        None,
        None,
        false,
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

    transactions::create_transaction(
        &pool,
        dec!(50),
        "Lunch",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 13),
    )
    .await
    .unwrap();

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

    categories::update_category(&pool, cat.id, "Groceries", CategoryType::Expense)
        .await
        .unwrap();

    let list = categories::list_categories(&pool).await.unwrap();
    assert_eq!(list[0].name, "Groceries");
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

    transactions::create_transaction(
        &pool,
        dec!(10),
        "Test",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await
    .unwrap();

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

    let txn = transactions::create_transaction(
        &pool,
        dec!(99.90),
        "Supermarket",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Debit,
        date(2026, 3, 10),
    )
    .await
    .unwrap();

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
        transactions::create_transaction(
            &pool,
            dec!(10),
            &format!("Day {day}"),
            cat.id,
            acc.id,
            TransactionType::Expense,
            PaymentMethod::Pix,
            date(2026, 3, day),
        )
        .await
        .unwrap();
    }

    let filters = transactions::TransactionFilterParams::default();
    let list = transactions::list_filtered(&pool, &filters, 10, 0).await.unwrap();
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
        transactions::create_transaction(
            &pool,
            dec!(10),
            "test",
            cat.id,
            acc.id,
            TransactionType::Expense,
            PaymentMethod::Pix,
            date(2026, 3, day),
        )
        .await
        .unwrap();
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

    let txn = transactions::create_transaction(
        &pool,
        dec!(10),
        "Old",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await
    .unwrap();

    let updated = transactions::update_transaction(
        &pool,
        txn.id,
        dec!(25),
        "New",
        cat.id,
        acc.id,
        TransactionType::Income,
        PaymentMethod::Boleto,
        date(2026, 3, 5),
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

    let txn = transactions::create_transaction(
        &pool,
        dec!(10),
        "test",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 1),
    )
    .await
    .unwrap();

    transactions::delete_transaction(&pool, txn.id).await.unwrap();

    let filters = transactions::TransactionFilterParams::default();
    let list = transactions::list_filtered(&pool, &filters, 100, 0).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn has_transactions_today_true() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    transactions::create_transaction(
        &pool,
        dec!(10),
        "test",
        cat.id,
        acc.id,
        TransactionType::Expense,
        PaymentMethod::Pix,
        date(2026, 3, 13),
    )
    .await
    .unwrap();

    assert!(transactions::has_transactions_today(&pool, date(2026, 3, 13)).await.unwrap());
}

#[tokio::test]
async fn has_transactions_today_false() {
    let (_guard, pool) = setup().await;
    assert!(!transactions::has_transactions_today(&pool, date(2026, 3, 13)).await.unwrap());
}

#[tokio::test]
async fn sum_expenses_by_category_sums_only_expenses() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;
    let income_cat = make_income_category(&pool, "Salary").await;

    // Two expenses in Food
    transactions::create_transaction(
        &pool, dec!(30), "a", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Pix, date(2026, 3, 1),
    ).await.unwrap();
    transactions::create_transaction(
        &pool, dec!(20), "b", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Pix, date(2026, 3, 5),
    ).await.unwrap();

    // An income in a different category (should not count)
    transactions::create_transaction(
        &pool, dec!(1000), "salary", income_cat.id, acc.id,
        TransactionType::Income, PaymentMethod::Transfer, date(2026, 3, 1),
    ).await.unwrap();

    let sum = transactions::sum_expenses_by_category(
        &pool, cat.id, date(2026, 3, 1), date(2026, 3, 31),
    ).await.unwrap();

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
    transactions::create_transaction(
        &pool, dec!(1000), "salary", inc_cat.id, acc.id,
        TransactionType::Income, PaymentMethod::Pix, date(2026, 3, 1),
    ).await.unwrap();

    // Expense R$200 via debit
    transactions::create_transaction(
        &pool, dec!(200), "grocery", exp_cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Debit, date(2026, 3, 2),
    ).await.unwrap();

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
    transactions::create_transaction(
        &pool, dec!(1000), "salary", inc_cat.id, acc.id,
        TransactionType::Income, PaymentMethod::Pix, date(2026, 3, 1),
    ).await.unwrap();

    // Credit expense R$300 (does NOT reduce checking balance)
    transactions::create_transaction(
        &pool, dec!(300), "credit purchase", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, date(2026, 3, 2),
    ).await.unwrap();

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
    transactions::create_transaction(
        &pool, dec!(1000), "salary", inc_cat.id, acc_a.id,
        TransactionType::Income, PaymentMethod::Pix, date(2026, 3, 1),
    ).await.unwrap();

    // Transfer R$400 from A to B
    transfers::create_transfer(&pool, acc_a.id, acc_b.id, dec!(400), "test", date(2026, 3, 2))
        .await.unwrap();

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
    transactions::create_transaction(
        &pool, dec!(1000), "salary", inc_cat.id, acc.id,
        TransactionType::Income, PaymentMethod::Pix, date(2026, 3, 1),
    ).await.unwrap();

    // Credit expense R$300
    transactions::create_transaction(
        &pool, dec!(300), "credit buy", exp_cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, date(2026, 3, 2),
    ).await.unwrap();

    // Pay credit card bill R$300
    credit_card_payments::create_payment(&pool, acc.id, dec!(300), date(2026, 3, 20), "pay bill")
        .await.unwrap();

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

    transactions::create_transaction(
        &pool, dec!(500), "s", inc_cat.id, acc_a.id,
        TransactionType::Income, PaymentMethod::Pix, date(2026, 3, 1),
    ).await.unwrap();

    transactions::create_transaction(
        &pool, dec!(100), "cash", inc_cat.id, acc_b.id,
        TransactionType::Income, PaymentMethod::Cash, date(2026, 3, 1),
    ).await.unwrap();

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
        &pool, acc_a.id, acc_b.id, dec!(100), "between", date(2026, 3, 1),
    ).await.unwrap();

    assert_eq!(t.amount, dec!(100));
    assert_eq!(t.from_account_id, acc_a.id);
    assert_eq!(t.to_account_id, acc_b.id);
}

#[tokio::test]
async fn transfer_same_account_rejected() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;

    let result = transfers::create_transfer(
        &pool, acc.id, acc.id, dec!(100), "self", date(2026, 3, 1),
    ).await;

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
        &pool, acc.id, dec!(500), date(2026, 3, 20), "March bill",
    ).await.unwrap();

    assert_eq!(p.amount, dec!(500));
    assert_eq!(p.description, "March bill");
}

// ═══════════════════════════════════════
// BUDGETS
// ═══════════════════════════════════════

#[tokio::test]
async fn create_budget_returns_row() {
    let (_guard, pool) = setup().await;
    let cat = make_expense_category(&pool, "Food").await;

    let b = budgets::create_budget(&pool, cat.id, dec!(800), BudgetPeriod::Monthly)
        .await.unwrap();

    assert_eq!(b.category_id, cat.id);
    assert_eq!(b.amount, dec!(800));
    assert_eq!(b.period, "monthly");
}

#[tokio::test]
async fn duplicate_budget_rejected() {
    let (_guard, pool) = setup().await;
    let cat = make_expense_category(&pool, "Food").await;

    budgets::create_budget(&pool, cat.id, dec!(800), BudgetPeriod::Monthly)
        .await.unwrap();

    // Same category + period → UNIQUE violation
    let result = budgets::create_budget(&pool, cat.id, dec!(500), BudgetPeriod::Monthly).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn update_budget_changes_amount() {
    let (_guard, pool) = setup().await;
    let cat = make_expense_category(&pool, "Food").await;

    let b = budgets::create_budget(&pool, cat.id, dec!(800), BudgetPeriod::Monthly)
        .await.unwrap();

    let updated = budgets::update_budget(&pool, b.id, dec!(1200)).await.unwrap();
    assert_eq!(updated.amount, dec!(1200));
}

#[tokio::test]
async fn compute_all_spending_respects_period() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Food").await;

    // Monthly budget
    let budget = budgets::create_budget(&pool, cat.id, dec!(500), BudgetPeriod::Monthly)
        .await.unwrap();

    // Expense in March
    transactions::create_transaction(
        &pool, dec!(150), "groceries", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Pix, date(2026, 3, 5),
    ).await.unwrap();

    // Expense in February (outside monthly range for March)
    transactions::create_transaction(
        &pool, dec!(200), "old groceries", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Pix, date(2026, 2, 15),
    ).await.unwrap();

    let today = date(2026, 3, 13);
    let (weekly_start, _) = BudgetPeriod::Weekly.date_range(today);
    let (monthly_start, _) = BudgetPeriod::Monthly.date_range(today);
    let (yearly_start, _) = BudgetPeriod::Yearly.date_range(today);

    let spending = budgets::compute_all_spending(
        &pool, weekly_start, monthly_start, yearly_start, today,
    ).await.unwrap();

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
        &pool, dec!(300), 3, "Headphones", cat.id, acc.id, date(2026, 3, 10),
    ).await.unwrap();

    assert_eq!(purchase.total_amount, dec!(300));
    assert_eq!(purchase.installment_count, 3);

    let txns = installments::get_installment_transactions(&pool, purchase.id)
        .await.unwrap();

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
        &pool, dec!(100), 3, "Test", cat.id, acc.id, date(2026, 1, 1),
    ).await.unwrap();

    let txns = installments::get_installment_transactions(&pool, purchase.id)
        .await.unwrap();

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
        &pool, dec!(200), 2, "Keyboard", cat.id, acc.id, date(2026, 3, 1),
    ).await.unwrap();

    installments::delete_installment_purchase(&pool, purchase.id)
        .await.unwrap();

    // All generated transactions should be gone
    let filters = transactions::TransactionFilterParams::default();
    let txns = transactions::list_filtered(&pool, &filters, 100, 0).await.unwrap();
    assert!(txns.is_empty());
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
        &pool, dec!(29.90), "Netflix", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, Frequency::Monthly,
        date(2026, 4, 1),
    ).await.unwrap();

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
        &pool, dec!(30), "A", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, Frequency::Monthly,
        date(2026, 3, 12),
    ).await.unwrap();

    // Due tomorrow (not pending)
    recurring::create_recurring(
        &pool, dec!(30), "B", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, Frequency::Monthly,
        date(2026, 3, 14),
    ).await.unwrap();

    let pending = recurring::list_pending(&pool, date(2026, 3, 13)).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].description, "A");
}

#[tokio::test]
async fn update_recurring_changes_fields() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Subscriptions").await;

    let r = recurring::create_recurring(
        &pool, dec!(30), "Old", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, Frequency::Monthly,
        date(2026, 4, 1),
    ).await.unwrap();

    let updated = recurring::update_recurring(
        &pool, r.id, dec!(50), "New", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Pix, Frequency::Weekly,
        date(2026, 5, 1),
    ).await.unwrap();

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
        &pool, dec!(30), "Netflix", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, Frequency::Monthly,
        date(2026, 4, 1),
    ).await.unwrap();

    recurring::deactivate_recurring(&pool, r.id).await.unwrap();

    let list = recurring::list_recurring(&pool).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn advance_next_due_updates_date() {
    let (_guard, pool) = setup().await;
    let acc = make_checking(&pool, "Nubank").await;
    let cat = make_expense_category(&pool, "Subscriptions").await;

    let r = recurring::create_recurring(
        &pool, dec!(30), "Netflix", cat.id, acc.id,
        TransactionType::Expense, PaymentMethod::Credit, Frequency::Monthly,
        date(2026, 3, 1),
    ).await.unwrap();

    let new_due = recurring::compute_next_due(r.next_due, r.parsed_frequency());
    recurring::advance_next_due(&pool, r.id, new_due).await.unwrap();

    let list = recurring::list_recurring(&pool).await.unwrap();
    assert_eq!(list[0].next_due, date(2026, 4, 1));
}
