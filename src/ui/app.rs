//! Central application state and key dispatch (Elm/TEA architecture).
//!
//! [`App`] is the single source of truth: it owns the DB pool, cached data,
//! and per-screen state structs. The event loop in `main.rs` drives a
//! `draw → handle_key → tick` cycle where:
//!
//! - **`draw`** renders the current screen from `App` (pure read).
//! - **`handle_key`** dispatches input through three layers:
//!   1. Popup (if active) — consumes all keys.
//!   2. [`InputMode`] gate — `Normal` routes to screen handlers + global nav;
//!      `Editing` routes to the active form; `Filtering` routes to the filter bar.
//!   3. Screen-specific handler (e.g. `handle_accounts_key`).
//! - **`tick`** decrements status-message countdowns.
//!
//! Shared data (accounts, categories, balances, name maps) is cached at the
//! top level and refreshed after mutations via targeted `refresh_*` helpers.

use std::collections::HashMap;

use chrono::{Local, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{
    models::{
        Account, Budget, Category, CreditCardPayment, InstallmentPurchase, Notification,
        RecurringTransaction, Transaction, Transfer,
    },
    ui::{
        components::{help_popup::HelpPopup, popup::ConfirmPopup},
        screens::transactions::{TransactionFilter, TransactionForm},
    },
};

use super::i18n::Locale;
use super::screens::{
    accounts::AccountForm,
    budgets::BudgetForm,
    categories::CategoryForm,
    cc_payments::CcPaymentForm,
    cc_statements::{CreditCardStatement, StatementsView},
    recurring::RecurringForm,
    transactions::InstallmentForm,
    transfers::TransferForm,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Transactions,
    Accounts,
    Budgets,
    Categories,
    Recurring,
    Transfers,
    CreditCardPayments,
    CreditCardStatements,
}

impl Screen {
    pub const ALL: [Self; 9] = [
        Self::Dashboard,
        Self::Transactions,
        Self::Accounts,
        Self::Budgets,
        Self::Categories,
        Self::Recurring,
        Self::Transfers,
        Self::CreditCardPayments,
        Self::CreditCardStatements,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Transactions => "Transactions",
            Self::Accounts => "Accounts",
            Self::Budgets => "Budgets",
            Self::Categories => "Categories",
            Self::Recurring => "Recurring",
            Self::Transfers => "Transfers",
            Self::CreditCardPayments => "CC Payments",
            Self::CreditCardStatements => "CC Statements",
        }
    }

    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::Dashboard => "screen.dashboard",
            Self::Transactions => "screen.transactions",
            Self::Accounts => "screen.accounts",
            Self::Budgets => "screen.budgets",
            Self::Categories => "screen.categories",
            Self::Recurring => "screen.recurring",
            Self::Transfers => "screen.transfers",
            Self::CreditCardPayments => "screen.cc_payments",
            Self::CreditCardStatements => "screen.cc_statements",
        }
    }

    pub(super) fn index(self) -> usize {
        Self::ALL.iter().position(|&s| s == self).unwrap()
    }

    fn next(self) -> Self {
        let i = self.index();
        Self::ALL.get(i + 1).copied().unwrap_or(self)
    }

    fn prev(self) -> Self {
        let i = self.index();
        if i > 0 { Self::ALL[i - 1] } else { self }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
    Filtering,
}

pub enum ConfirmAction {
    DeactivateAccount(i32),
    DeleteCategory(i32),
    DeleteTransaction(i32),
    DeleteBudget(i32),
    DeleteInstallment(i32),
    DeactivateRecurring(i32),
    DeleteTransfer(i32),
    DeleteCreditCardPayment(i32),
    PayCreditCardStatement {
        account_id: i32,
        amount: Decimal,
        date: NaiveDate,
        description: String,
    },
    UnpayCreditCardStatement {
        account_id: i32,
        pay_start: NaiveDate,
        pay_end: NaiveDate,
    },
}

pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
    pub ticks_remaining: u8,
}

impl StatusMessage {
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
            ticks_remaining: 20, // 5 seconds at 250ms tick
        }
    }
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
            ticks_remaining: 12, // 3 seconds at 250ms tick
        }
    }
}

pub(crate) const PAGE_SIZE: u64 = 100;

// ── Per-screen state structs ──────────────────────────────────────

pub struct DashboardState {
    pub current_statements: Vec<(String, CreditCardStatement)>,
    pub notifications: Vec<Notification>,
    pub notification_selection: usize,
}

pub struct TransactionScreenState {
    pub items: Vec<Transaction>,
    pub table_state: TableState,
    pub form: Option<TransactionForm>,
    pub inst_form: Option<InstallmentForm>,
    pub offset: u64,
    pub count: u64,
    pub filter: TransactionFilter,
}

pub struct AccountScreenState {
    pub table_state: TableState,
    pub form: Option<AccountForm>,
}

pub struct CategoryScreenState {
    pub table_state: TableState,
    pub form: Option<CategoryForm>,
}

pub struct BudgetScreenState {
    pub items: Vec<Budget>,
    pub spent: HashMap<i32, Decimal>,
    pub table_state: TableState,
    pub form: Option<BudgetForm>,
}

pub struct RecurringScreenState {
    pub pending: Vec<RecurringTransaction>,
    pub list: Vec<RecurringTransaction>,
    pub table_state: TableState,
    pub form: Option<RecurringForm>,
}

pub struct TransferScreenState {
    pub items: Vec<Transfer>,
    pub table_state: TableState,
    pub form: Option<TransferForm>,
    pub offset: u64,
    pub count: u64,
}

pub struct CcPaymentScreenState {
    pub items: Vec<CreditCardPayment>,
    pub table_state: TableState,
    pub form: Option<CcPaymentForm>,
    pub offset: u64,
    pub count: u64,
}

pub struct CcStatementScreenState {
    pub items: Vec<CreditCardStatement>,
    pub table_state: TableState,
    pub account_idx: usize,
    pub view: StatementsView,
    pub detail_txns: Vec<Transaction>,
    pub detail_table_state: TableState,
}

// ── Central application state ─────────────────────────────────────

/// Single source of truth for the TUI.
/// Per-screen state is grouped into dedicated structs for clarity.
pub struct App {
    pub running: bool,
    pub screen: Screen,
    pub pool: PgPool,
    pub is_prod: bool,
    pub input_mode: InputMode,
    pub locale: Locale,
    pub help_popup: Option<HelpPopup>,
    pub confirm_popup: Option<ConfirmPopup>,
    pub confirm_action: Option<ConfirmAction>,
    pub status_message: Option<StatusMessage>,
    /// Tracks pending 'g' keypress for vim-style `gg` (jump to first row).
    pub pending_g: bool,

    // Shared data used across screens
    pub accounts: Vec<Account>,
    pub categories: Vec<Category>,
    pub balances: HashMap<i32, (Decimal, Decimal)>,
    pub account_names: HashMap<i32, String>,
    pub category_names: HashMap<i32, String>,

    // Installment purchases cache (used by transactions screen for group edit)
    pub installment_purchases: Vec<InstallmentPurchase>,

    // Per-screen state
    pub dashboard: DashboardState,
    pub txn: TransactionScreenState,
    pub acct: AccountScreenState,
    pub cat: CategoryScreenState,
    pub budget: BudgetScreenState,
    pub recur: RecurringScreenState,
    pub xfer: TransferScreenState,
    pub cc_pay: CcPaymentScreenState,
    pub cc_stmt: CcStatementScreenState,
}

impl App {
    pub fn new(pool: PgPool, is_prod: bool) -> Self {
        Self {
            running: true,
            screen: Screen::Dashboard,
            pool,
            is_prod,
            locale: Locale::default(),
            help_popup: None,
            input_mode: InputMode::Normal,
            confirm_popup: None,
            confirm_action: None,
            status_message: None,
            pending_g: false,

            accounts: Vec::new(),
            categories: Vec::new(),
            balances: HashMap::new(),
            account_names: HashMap::new(),
            category_names: HashMap::new(),
            installment_purchases: Vec::new(),

            dashboard: DashboardState {
                current_statements: Vec::new(),
                notifications: Vec::new(),
                notification_selection: 0,
            },
            txn: TransactionScreenState {
                items: Vec::new(),
                table_state: TableState::default().with_selected(0),
                form: None,
                inst_form: None,
                offset: 0,
                count: 0,
                filter: TransactionFilter::new(Locale::default()),
            },
            acct: AccountScreenState {
                table_state: TableState::default().with_selected(0),
                form: None,
            },
            cat: CategoryScreenState {
                table_state: TableState::default().with_selected(0),
                form: None,
            },
            budget: BudgetScreenState {
                items: Vec::new(),
                spent: HashMap::new(),
                table_state: TableState::default().with_selected(0),
                form: None,
            },
            recur: RecurringScreenState {
                pending: Vec::new(),
                list: Vec::new(),
                table_state: TableState::default().with_selected(0),
                form: None,
            },
            xfer: TransferScreenState {
                items: Vec::new(),
                table_state: TableState::default().with_selected(0),
                form: None,
                offset: 0,
                count: 0,
            },
            cc_pay: CcPaymentScreenState {
                items: Vec::new(),
                table_state: TableState::default().with_selected(0),
                form: None,
                offset: 0,
                count: 0,
            },
            cc_stmt: CcStatementScreenState {
                items: Vec::new(),
                table_state: TableState::default().with_selected(0),
                account_idx: 0,
                view: StatementsView::List,
                detail_txns: Vec::new(),
                detail_table_state: TableState::default().with_selected(0),
            },
        }
    }

    pub fn account_name(&self, id: i32) -> &str {
        self.account_names
            .get(&id)
            .map(|s| s.as_str())
            .unwrap_or("?")
    }

    pub fn category_name(&self, id: i32) -> &str {
        self.category_names
            .get(&id)
            .map(|s| s.as_str())
            .unwrap_or("?")
    }

    /// Return the category name in the active locale.
    /// Fallback chain: PT `name_pt` → EN `name` → "?".
    pub fn category_name_localized(&self, id: i32) -> &str {
        if self.locale == Locale::Pt
            && let Some(cat) = self.categories.iter().find(|c| c.id == id)
            && let Some(pt) = &cat.name_pt
        {
            return pt.as_str();
        }
        self.category_name(id)
    }

    pub async fn load_data(&mut self) -> anyhow::Result<()> {
        use crate::db::{
            accounts, budgets, categories, credit_card_payments, installments, notifications,
            recurring, transfers,
        };
        use crate::models::BudgetPeriod;

        let today = Local::now().date_naive();
        let (weekly_start, _) = BudgetPeriod::Weekly.date_range(today);
        let (monthly_start, _) = BudgetPeriod::Monthly.date_range(today);
        let (yearly_start, _) = BudgetPeriod::Yearly.date_range(today);

        let pool = &self.pool;
        let transfer_offset = self.xfer.offset as i64;
        let cc_payment_offset = self.cc_pay.offset as i64;

        let (
            accts,
            cats,
            bals,
            acct_names,
            budgets_list,
            budget_spent_map,
            pending,
            rec_list,
            inst_list,
            transfer_list,
            transfer_cnt,
            cc_list,
            cc_cnt,
            notifs,
        ) = tokio::try_join!(
            accounts::list_accounts(pool),
            categories::list_categories(pool),
            accounts::compute_all_balances(pool),
            accounts::list_all_account_names(pool),
            budgets::list_budgets(pool),
            budgets::compute_all_spending(pool, weekly_start, monthly_start, yearly_start, today),
            recurring::list_pending(pool, today),
            recurring::list_recurring(pool),
            installments::list_installment_purchases(pool),
            transfers::list_transfers(pool, PAGE_SIZE as i64, transfer_offset),
            transfers::count_transfers(pool),
            credit_card_payments::list_all_payments(pool, PAGE_SIZE as i64, cc_payment_offset),
            credit_card_payments::count_payments(pool),
            notifications::list_unread(pool),
        )?;

        self.accounts = accts;
        self.categories = cats;
        self.balances = bals;
        self.account_names = acct_names;
        self.category_names = self
            .categories
            .iter()
            .map(|c| (c.id, c.name.clone()))
            .collect();
        self.budget.items = budgets_list;
        self.budget.spent = budget_spent_map;
        self.recur.pending = pending;
        self.recur.list = rec_list;
        self.installment_purchases = inst_list;
        self.xfer.items = transfer_list;
        self.xfer.count = transfer_cnt;
        self.cc_pay.items = cc_list;
        self.cc_pay.count = cc_cnt;
        self.dashboard.notifications = notifs;

        self.load_transactions().await?;

        // Compute current CC statements for dashboard
        self.dashboard.current_statements = self.compute_current_statements().await?;

        Ok(())
    }

    /// Build the current (open) CC statement for each credit card account.
    /// Used by the dashboard to show a summary; uses a single batched query.
    async fn compute_current_statements(
        &self,
    ) -> anyhow::Result<Vec<(String, CreditCardStatement)>> {
        use crate::db::{
            clamped_day, latest_closing_date, next_month, statement_due_date, transactions,
        };
        use chrono::Datelike;

        let today = Local::now().date_naive();

        struct AccInfo {
            name: String,
            id: i32,
            current_start: NaiveDate,
            next_close: NaiveDate,
            due: NaiveDate,
        }

        let mut acc_infos = Vec::new();
        let mut ranges = Vec::new();

        for acc in &self.accounts {
            if !acc.has_credit_card {
                continue;
            }
            let billing_day = acc.billing_day.unwrap_or(1) as u32;
            let due_day = acc.due_day.unwrap_or(1) as u32;

            let latest_close = latest_closing_date(today, billing_day);
            let current_start = latest_close.succ_opt().unwrap();

            let (next_y, next_m) = next_month(latest_close.year(), latest_close.month());
            let next_close = clamped_day(next_y, next_m, billing_day);
            let due = statement_due_date(next_y, next_m, billing_day, due_day);

            ranges.push((acc.id, current_start, next_close));
            acc_infos.push(AccInfo {
                name: acc.name.clone(),
                id: acc.id,
                current_start,
                next_close,
                due,
            });
        }

        let sums = transactions::sum_credit_by_accounts_batch(&self.pool, &ranges).await?;

        let mut results = Vec::with_capacity(acc_infos.len());
        for info in acc_infos {
            let (charges, credits) = sums.get(&info.id).copied().unwrap_or_default();
            let total = charges - credits;
            results.push((
                info.name,
                CreditCardStatement {
                    period_start: info.current_start,
                    period_end: info.next_close,
                    due_date: info.due,
                    total_charges: charges,
                    total_credits: credits,
                    statement_total: total,
                    paid_amount: Decimal::ZERO,
                    is_current: true,
                    is_upcoming: false,
                },
            ));
        }

        Ok(results)
    }

    pub async fn load_transfers(&mut self) -> anyhow::Result<()> {
        self.xfer.items = crate::db::transfers::list_transfers(
            &self.pool,
            PAGE_SIZE as i64,
            self.xfer.offset as i64,
        )
        .await?;
        self.xfer.count = crate::db::transfers::count_transfers(&self.pool).await?;
        Ok(())
    }

    pub async fn load_cc_payments(&mut self) -> anyhow::Result<()> {
        self.cc_pay.items = crate::db::credit_card_payments::list_all_payments(
            &self.pool,
            PAGE_SIZE as i64,
            self.cc_pay.offset as i64,
        )
        .await?;
        self.cc_pay.count = crate::db::credit_card_payments::count_payments(&self.pool).await?;
        Ok(())
    }

    // ── Targeted refresh helpers ────────────────────────────────────

    pub async fn refresh_accounts(&mut self) -> anyhow::Result<()> {
        use crate::db::accounts;
        let (accts, names, bals) = tokio::try_join!(
            accounts::list_accounts(&self.pool),
            accounts::list_all_account_names(&self.pool),
            accounts::compute_all_balances(&self.pool),
        )?;
        self.accounts = accts;
        self.account_names = names;
        self.balances = bals;
        Ok(())
    }

    pub async fn refresh_balances(&mut self) -> anyhow::Result<()> {
        self.balances = crate::db::accounts::compute_all_balances(&self.pool).await?;
        Ok(())
    }

    pub async fn refresh_categories(&mut self) -> anyhow::Result<()> {
        self.categories = crate::db::categories::list_categories(&self.pool).await?;
        self.category_names = self
            .categories
            .iter()
            .map(|c| (c.id, c.name.clone()))
            .collect();
        Ok(())
    }

    pub async fn refresh_budgets(&mut self) -> anyhow::Result<()> {
        use crate::db::budgets;
        use crate::models::BudgetPeriod;
        let today = Local::now().date_naive();
        let (weekly_start, _) = BudgetPeriod::Weekly.date_range(today);
        let (monthly_start, _) = BudgetPeriod::Monthly.date_range(today);
        let (yearly_start, _) = BudgetPeriod::Yearly.date_range(today);
        let (bl, bs) = tokio::try_join!(
            budgets::list_budgets(&self.pool),
            budgets::compute_all_spending(&self.pool, weekly_start, monthly_start, yearly_start, today),
        )?;
        self.budget.items = bl;
        self.budget.spent = bs;
        Ok(())
    }

    pub async fn refresh_recurring(&mut self) -> anyhow::Result<()> {
        let today = Local::now().date_naive();
        let (pending, list) = tokio::try_join!(
            crate::db::recurring::list_pending(&self.pool, today),
            crate::db::recurring::list_recurring(&self.pool),
        )?;
        self.recur.pending = pending;
        self.recur.list = list;
        Ok(())
    }

    pub async fn refresh_installments(&mut self) -> anyhow::Result<()> {
        self.installment_purchases =
            crate::db::installments::list_installment_purchases(&self.pool).await?;
        Ok(())
    }

    pub async fn refresh_notifications(&mut self) -> anyhow::Result<()> {
        self.dashboard.notifications = crate::db::notifications::list_unread(&self.pool).await?;
        Ok(())
    }

    pub async fn refresh_dashboard_statements(&mut self) -> anyhow::Result<()> {
        self.dashboard.current_statements = self.compute_current_statements().await?;
        Ok(())
    }

    pub fn tick(&mut self) {
        if let Some(msg) = &mut self.status_message {
            msg.ticks_remaining = msg.ticks_remaining.saturating_sub(1);
            if msg.ticks_remaining == 0 {
                self.status_message = None;
            }
        }
    }

    pub async fn load_transactions(&mut self) -> anyhow::Result<()> {
        use crate::db::transactions;

        let params = self
            .txn
            .filter
            .to_params(&self.accounts, &self.categories);
        self.txn.items = transactions::list_filtered(
            &self.pool,
            &params,
            PAGE_SIZE as i64,
            self.txn.offset as i64,
        )
        .await?;
        self.txn.count = transactions::count_filtered(&self.pool, &params).await?;
        Ok(())
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        // Popup takes priority over everything
        if let Some(popup) = &mut self.confirm_popup {
            if let Some(confirmed) = popup.handle_key(key.code) {
                if confirmed {
                    match self.confirm_action.take() {
                        Some(ConfirmAction::DeactivateAccount(id)) => {
                            self.execute_deactivate(id).await?;
                        }
                        Some(ConfirmAction::DeleteCategory(id)) => {
                            self.execute_delete_category(id).await?;
                        }
                        Some(ConfirmAction::DeleteTransaction(id)) => {
                            self.execute_delete_transaction(id).await?;
                        }
                        Some(ConfirmAction::DeleteBudget(id)) => {
                            self.execute_delete_budget(id).await?;
                        }
                        Some(ConfirmAction::DeleteInstallment(id)) => {
                            self.execute_delete_installment(id).await?;
                        }
                        Some(ConfirmAction::DeactivateRecurring(id)) => {
                            self.execute_deactivate_recurring(id).await?;
                        }
                        Some(ConfirmAction::DeleteTransfer(id)) => {
                            self.execute_delete_transfer(id).await?;
                        }
                        Some(ConfirmAction::DeleteCreditCardPayment(id)) => {
                            self.execute_delete_cc_payment(id).await?;
                        }
                        Some(ConfirmAction::PayCreditCardStatement {
                            account_id,
                            amount,
                            date,
                            description,
                        }) => {
                            self.execute_pay_cc_statement(account_id, amount, date, &description)
                                .await?;
                        }
                        Some(ConfirmAction::UnpayCreditCardStatement {
                            account_id,
                            pay_start,
                            pay_end,
                        }) => {
                            self.execute_unpay_cc_statement(account_id, pay_start, pay_end)
                                .await?;
                        }
                        None => {}
                    }
                }
                self.confirm_popup = None;
                self.confirm_action = None;
            }
            return Ok(());
        }

        // Help popup takes second priority
        if let Some(popup) = &mut self.help_popup {
            if popup.handle_key(key.code) {
                self.help_popup = None;
            }
            return Ok(());
        }

        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key).await,
            InputMode::Editing => self.handle_editing_key(key).await,
            InputMode::Filtering => self.handle_filtering_key(key).await,
        }
    }

    // ── Normal-mode dispatch ──────────────────────────────────────────

    async fn handle_normal_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        // Handle pending 'g' for gg (jump to first row)
        if self.pending_g {
            self.pending_g = false;
            if key.code == KeyCode::Char('g') {
                self.jump_to_row(0);
                return Ok(());
            }
            // Not 'g' — fall through to process the key normally
        }

        // Ctrl+d / Ctrl+u — half-page scroll (10 rows)
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('d') => {
                    if let Some((state, len)) = self.active_table_state() {
                        move_table_selection(state, len, 10);
                    }
                    return Ok(());
                }
                KeyCode::Char('u') => {
                    if let Some((state, len)) = self.active_table_state() {
                        move_table_selection(state, len, -10);
                    }
                    return Ok(());
                }
                KeyCode::Char('l') => {
                    self.locale = self.locale.toggle();
                    return Ok(());
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char(c @ '1'..='9') => {
                let i = (c as usize) - ('1' as usize);
                if let Some(&screen) = Screen::ALL.get(i) {
                    self.screen = screen;
                    if screen == Screen::CreditCardStatements {
                        self.load_cc_statements().await?;
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h')
                if key.code == KeyCode::Left || self.screen != Screen::CreditCardStatements =>
            {
                self.screen = self.screen.prev();
                if self.screen == Screen::CreditCardStatements {
                    self.load_cc_statements().await?;
                }
            }
            KeyCode::Right | KeyCode::Char('l')
                if key.code == KeyCode::Right || self.screen != Screen::CreditCardStatements =>
            {
                self.screen = self.screen.next();
                if self.screen == Screen::CreditCardStatements {
                    self.load_cc_statements().await?;
                }
            }
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Char('G') => {
                self.jump_to_last_row();
            }
            KeyCode::Char('?') => {
                self.help_popup = Some(HelpPopup::new(self.screen));
            }
            _ => match self.screen {
                Screen::Dashboard => self.handle_dashboard_key(key.code).await?,
                Screen::Accounts => self.handle_accounts_key(key.code).await?,
                Screen::Categories => self.handle_categories_key(key.code).await?,
                Screen::Transactions => self.handle_transactions_key(key.code).await?,
                Screen::Budgets => self.handle_budgets_key(key.code).await?,
                Screen::Recurring => self.handle_recurring_key(key.code).await?,
                Screen::Transfers => self.handle_transfers_key(key.code).await?,
                Screen::CreditCardPayments => self.handle_cc_payments_key(key.code).await?,
                Screen::CreditCardStatements => self.handle_cc_statements_key(key.code).await?,
            },
        }
        Ok(())
    }

    /// Returns a mutable reference to the active screen's table state and its list length.
    /// Dashboard uses notification_selection (not TableState), so returns None.
    fn active_table_state(&mut self) -> Option<(&mut TableState, usize)> {
        match self.screen {
            Screen::Dashboard => None,
            Screen::Transactions => {
                Some((&mut self.txn.table_state, self.txn.items.len()))
            }
            Screen::Accounts => Some((&mut self.acct.table_state, self.accounts.len())),
            Screen::Budgets => Some((&mut self.budget.table_state, self.budget.items.len())),
            Screen::Categories => Some((&mut self.cat.table_state, self.categories.len())),
            Screen::Recurring => Some((&mut self.recur.table_state, self.recur.list.len())),
            Screen::Transfers => Some((&mut self.xfer.table_state, self.xfer.items.len())),
            Screen::CreditCardPayments => {
                Some((&mut self.cc_pay.table_state, self.cc_pay.items.len()))
            }
            Screen::CreditCardStatements => match self.cc_stmt.view {
                StatementsView::List => {
                    Some((&mut self.cc_stmt.table_state, self.cc_stmt.items.len()))
                }
                StatementsView::Detail => Some((
                    &mut self.cc_stmt.detail_table_state,
                    self.cc_stmt.detail_txns.len(),
                )),
            },
        }
    }

    fn jump_to_row(&mut self, row: usize) {
        if self.screen == Screen::Dashboard {
            if !self.dashboard.notifications.is_empty() {
                self.dashboard.notification_selection = row.min(self.dashboard.notifications.len() - 1);
            }
        } else if let Some((state, len)) = self.active_table_state()
            && len > 0
        {
            state.select(Some(row.min(len - 1)));
        }
    }

    fn jump_to_last_row(&mut self) {
        if self.screen == Screen::Dashboard {
            if !self.dashboard.notifications.is_empty() {
                self.dashboard.notification_selection = self.dashboard.notifications.len() - 1;
            }
        } else if let Some((state, len)) = self.active_table_state()
            && len > 0
        {
            state.select(Some(len - 1));
        }
    }

    // ── Editing-mode dispatch ─────────────────────────────────────────

    async fn handle_editing_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if key.code == KeyCode::Esc {
            // If installment confirmation is showing, dismiss confirmation only
            if let Some(form) = &mut self.txn.inst_form
                && form.confirmation.is_some()
            {
                form.confirmation = None;
                return Ok(());
            }
            if let Some(form) = &mut self.txn.form
                && form.confirmation.is_some()
            {
                form.confirmation = None;
                return Ok(());
            }
            self.acct.form = None;
            self.cat.form = None;
            self.txn.form = None;
            self.txn.inst_form = None;
            self.budget.form = None;
            self.recur.form = None;
            self.xfer.form = None;
            self.cc_pay.form = None;
            self.input_mode = InputMode::Normal;
            return Ok(());
        }
        if key.code == KeyCode::Enter {
            if self.acct.form.is_some() {
                self.submit_account_form().await?;
            } else if self.cat.form.is_some() {
                self.submit_category_form().await?;
            } else if self.txn.inst_form.is_some() {
                self.submit_installment_form().await?;
            } else if self.txn.form.is_some() {
                self.submit_transaction_form().await?;
            } else if self.budget.form.is_some() {
                self.submit_budget_form().await?;
            } else if self.recur.form.is_some() {
                self.submit_recurring_form().await?;
            } else if self.xfer.form.is_some() {
                self.submit_transfer_form().await?;
            } else if self.cc_pay.form.is_some() {
                self.submit_cc_payment_form().await?;
            }
            return Ok(());
        }

        if self.acct.form.is_some() {
            self.handle_account_form_key(key.code);
        } else if self.cat.form.is_some() {
            self.handle_category_form_key(key.code);
        } else if self.txn.inst_form.is_some() {
            self.handle_installment_form_key(key.code);
        } else if self.txn.form.is_some() {
            self.handle_transaction_form_key(key.code);
        } else if self.budget.form.is_some() {
            self.handle_budget_form_key(key.code);
        } else if self.recur.form.is_some() {
            self.handle_recurring_form_key(key.code);
        } else if self.xfer.form.is_some() {
            self.handle_transfer_form_key(key.code);
        } else if self.cc_pay.form.is_some() {
            self.handle_cc_payment_form_key(key.code);
        }
        Ok(())
    }

    // ── Dashboard key handlers ────────────────────────────────────────

    async fn handle_dashboard_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        if self.dashboard.notifications.is_empty() {
            return Ok(());
        }
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.dashboard.notification_selection > 0 {
                    self.dashboard.notification_selection -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.dashboard.notifications.len().saturating_sub(1);
                if self.dashboard.notification_selection < max {
                    self.dashboard.notification_selection += 1;
                }
            }
            KeyCode::Char('r') => {
                if let Some(n) = self.dashboard.notifications.get(self.dashboard.notification_selection) {
                    let id = n.id;
                    crate::db::notifications::mark_read(&self.pool, id).await?;
                    self.refresh_notifications().await?;
                    // Clamp selection after removal
                    if !self.dashboard.notifications.is_empty() {
                        self.dashboard.notification_selection = self
                            .dashboard
                            .notification_selection
                            .min(self.dashboard.notifications.len() - 1);
                    } else {
                        self.dashboard.notification_selection = 0;
                    }
                }
            }
            KeyCode::Char('R') => {
                crate::db::notifications::mark_all_read(&self.pool).await?;
                self.refresh_notifications().await?;
                self.dashboard.notification_selection = 0;
            }
            _ => {}
        }
        Ok(())
    }
}

pub(crate) fn move_table_selection(state: &mut TableState, len: usize, delta: isize) {
    let i = state.selected().unwrap_or(0);
    let new = (i as isize + delta).clamp(0, len.saturating_sub(1) as isize) as usize;
    state.select(Some(new));
}

pub(crate) fn clamp_selection(state: &mut TableState, len: usize) {
    if len == 0 {
        state.select(None);
    } else {
        let max = len - 1;
        let i = state.selected().unwrap_or(0);
        state.select(Some(i.min(max)));
    }
}

pub(crate) fn is_toggle_key(code: KeyCode) -> bool {
    matches!(code, KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right)
}

pub(crate) fn cycle_index(idx: &mut usize, len: usize, code: KeyCode) {
    if len > 0 {
        *idx = match code {
            KeyCode::Left | KeyCode::Char('h') => (*idx + len - 1) % len,
            _ => (*idx + 1) % len, // Space, Right, 'l' go forward
        };
    }
}
