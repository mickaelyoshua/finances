use std::collections::HashMap;

use chrono::Local;
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
        components::popup::ConfirmPopup,
        screens::transactions::{TransactionFilter, TransactionForm},
    },
};

use super::screens::{
    accounts::AccountForm, budgets::BudgetForm, categories::CategoryForm,
    cc_payments::CcPaymentForm, installments::InstallmentForm, recurring::RecurringForm,
    transfers::TransferForm,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Transactions,
    Accounts,
    Budgets,
    Categories,
    Installments,
    Recurring,
    Transfers,
    CreditCardPayments,
}

impl Screen {
    pub const ALL: [Self; 9] = [
        Self::Dashboard,
        Self::Transactions,
        Self::Accounts,
        Self::Budgets,
        Self::Categories,
        Self::Installments,
        Self::Recurring,
        Self::Transfers,
        Self::CreditCardPayments,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Transactions => "Transactions",
            Self::Accounts => "Accounts",
            Self::Budgets => "Budgets",
            Self::Categories => "Categories",
            Self::Installments => "Installments",
            Self::Recurring => "Recurring",
            Self::Transfers => "Transfers",
            Self::CreditCardPayments => "Credit Card Payments",
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

/// Central application state — single source of truth for the TUI.
///
/// Caches all DB data in memory and refreshes after every mutation via `load_data()`.
/// Owns form state, table selection state, and popup state for all seven screens.
/// Passed as `&mut` to both key handlers and render functions because ratatui's
/// `render_stateful_widget` requires mutable access to `TableState`.
pub struct App {
    pub running: bool,
    pub screen: Screen,
    pub pool: PgPool,
    pub is_prod: bool,
    pub accounts: Vec<Account>,
    pub categories: Vec<Category>,
    pub balances: HashMap<i32, (Decimal, Decimal)>, // account_id -> (checking, credit_used)
    pub account_names: HashMap<i32, String>,        // all accounts (incl. inactive) for lookups
    pub budgets: Vec<Budget>,
    pub budget_spent: HashMap<i32, Decimal>, // budget_id -> spent in current period
    pub budget_table_state: TableState,
    pub budget_form: Option<BudgetForm>,

    pub account_table_state: TableState,
    pub account_form: Option<AccountForm>,

    pub input_mode: InputMode,
    pub confirm_popup: Option<ConfirmPopup>,
    pub confirm_action: Option<ConfirmAction>,

    pub category_table_state: TableState,
    pub category_form: Option<CategoryForm>,

    pub transactions: Vec<Transaction>,
    pub transaction_table_state: TableState,
    pub transaction_form: Option<TransactionForm>,
    pub transaction_offset: u64,
    pub transaction_count: u64,
    pub transaction_filter: TransactionFilter,

    pub installments: Vec<InstallmentPurchase>,
    pub installment_table_state: TableState,
    pub installment_form: Option<InstallmentForm>,

    pub pending_recurring: Vec<RecurringTransaction>,
    pub recurring_list: Vec<RecurringTransaction>,
    pub recurring_table_state: TableState,
    pub recurring_form: Option<RecurringForm>,

    pub transfers: Vec<Transfer>,
    pub transfer_table_state: TableState,
    pub transfer_form: Option<TransferForm>,
    pub transfer_offset: u64,
    pub transfer_count: u64,

    pub cc_payments: Vec<CreditCardPayment>,
    pub cc_payment_table_state: TableState,
    pub cc_payment_form: Option<CcPaymentForm>,
    pub cc_payment_offset: u64,
    pub cc_payment_count: u64,

    pub category_names: HashMap<i32, String>,

    pub notifications: Vec<Notification>,
    pub notification_selection: usize,

    pub status_message: Option<StatusMessage>,

    /// Tracks pending 'g' keypress for vim-style `gg` (jump to first row).
    pub pending_g: bool,
}

impl App {
    pub fn new(pool: PgPool, is_prod: bool) -> Self {
        Self {
            running: true,
            screen: Screen::Dashboard,
            pool,
            is_prod,
            accounts: Vec::new(),
            categories: Vec::new(),
            balances: HashMap::new(),
            account_names: HashMap::new(),
            budgets: Vec::new(),
            budget_spent: HashMap::new(),
            budget_table_state: TableState::default().with_selected(0),
            budget_form: None,

            account_table_state: TableState::default().with_selected(0),
            account_form: None,

            input_mode: InputMode::Normal,
            confirm_popup: None,
            confirm_action: None,

            category_table_state: TableState::default().with_selected(0),
            category_form: None,

            transactions: Vec::new(),
            transaction_table_state: TableState::default().with_selected(0),
            transaction_form: None,
            transaction_offset: 0,
            transaction_count: 0,
            transaction_filter: TransactionFilter::new(),

            installments: Vec::new(),
            installment_table_state: TableState::default().with_selected(0),
            installment_form: None,

            pending_recurring: Vec::new(),
            recurring_list: Vec::new(),
            recurring_table_state: TableState::default().with_selected(0),
            recurring_form: None,

            transfers: Vec::new(),
            transfer_table_state: TableState::default().with_selected(0),
            transfer_form: None,
            transfer_offset: 0,
            transfer_count: 0,

            cc_payments: Vec::new(),
            cc_payment_table_state: TableState::default().with_selected(0),
            cc_payment_form: None,
            cc_payment_offset: 0,
            cc_payment_count: 0,

            category_names: HashMap::new(),

            notifications: Vec::new(),
            notification_selection: 0,

            status_message: None,
            pending_g: false,
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
        let transfer_offset = self.transfer_offset as i64;
        let cc_payment_offset = self.cc_payment_offset as i64;

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
        self.budgets = budgets_list;
        self.budget_spent = budget_spent_map;
        self.pending_recurring = pending;
        self.recurring_list = rec_list;
        self.installments = inst_list;
        self.transfers = transfer_list;
        self.transfer_count = transfer_cnt;
        self.cc_payments = cc_list;
        self.cc_payment_count = cc_cnt;
        self.notifications = notifs;

        self.load_transactions().await?;

        Ok(())
    }

    pub async fn load_transfers(&mut self) -> anyhow::Result<()> {
        self.transfers = crate::db::transfers::list_transfers(
            &self.pool,
            PAGE_SIZE as i64,
            self.transfer_offset as i64,
        )
        .await?;
        self.transfer_count = crate::db::transfers::count_transfers(&self.pool).await?;
        Ok(())
    }

    pub async fn load_cc_payments(&mut self) -> anyhow::Result<()> {
        self.cc_payments = crate::db::credit_card_payments::list_all_payments(
            &self.pool,
            PAGE_SIZE as i64,
            self.cc_payment_offset as i64,
        )
        .await?;
        self.cc_payment_count = crate::db::credit_card_payments::count_payments(&self.pool).await?;
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
            .transaction_filter
            .to_params(&self.accounts, &self.categories);
        self.transactions = transactions::list_filtered(
            &self.pool,
            &params,
            PAGE_SIZE as i64,
            self.transaction_offset as i64,
        )
        .await?;
        self.transaction_count = transactions::count_filtered(&self.pool, &params).await?;
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
                        None => {}
                    }
                }
                self.confirm_popup = None;
                self.confirm_action = None;
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
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char(c @ '1'..='9') => {
                let i = (c as usize) - ('1' as usize);
                if let Some(&screen) = Screen::ALL.get(i) {
                    self.screen = screen;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => self.screen = self.screen.prev(),
            KeyCode::Right | KeyCode::Char('l') => self.screen = self.screen.next(),
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Char('G') => {
                self.jump_to_last_row();
            }
            _ => match self.screen {
                Screen::Dashboard => self.handle_dashboard_key(key.code).await?,
                Screen::Accounts => self.handle_accounts_key(key.code).await?,
                Screen::Categories => self.handle_categories_key(key.code).await?,
                Screen::Transactions => self.handle_transactions_key(key.code).await?,
                Screen::Budgets => self.handle_budgets_key(key.code).await?,
                Screen::Installments => self.handle_installments_key(key.code).await?,
                Screen::Recurring => self.handle_recurring_key(key.code).await?,
                Screen::Transfers => self.handle_transfers_key(key.code).await?,
                Screen::CreditCardPayments => self.handle_cc_payments_key(key.code).await?,
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
                Some((&mut self.transaction_table_state, self.transactions.len()))
            }
            Screen::Accounts => Some((&mut self.account_table_state, self.accounts.len())),
            Screen::Budgets => Some((&mut self.budget_table_state, self.budgets.len())),
            Screen::Categories => Some((&mut self.category_table_state, self.categories.len())),
            Screen::Installments => {
                Some((&mut self.installment_table_state, self.installments.len()))
            }
            Screen::Recurring => Some((&mut self.recurring_table_state, self.recurring_list.len())),
            Screen::Transfers => Some((&mut self.transfer_table_state, self.transfers.len())),
            Screen::CreditCardPayments => {
                Some((&mut self.cc_payment_table_state, self.cc_payments.len()))
            }
        }
    }

    fn jump_to_row(&mut self, row: usize) {
        if self.screen == Screen::Dashboard {
            if !self.notifications.is_empty() {
                self.notification_selection = row.min(self.notifications.len() - 1);
            }
        } else if let Some((state, len)) = self.active_table_state()
            && len > 0
        {
            state.select(Some(row.min(len - 1)));
        }
    }

    fn jump_to_last_row(&mut self) {
        if self.screen == Screen::Dashboard {
            if !self.notifications.is_empty() {
                self.notification_selection = self.notifications.len() - 1;
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
            self.account_form = None;
            self.category_form = None;
            self.transaction_form = None;
            self.budget_form = None;
            self.installment_form = None;
            self.recurring_form = None;
            self.transfer_form = None;
            self.cc_payment_form = None;
            self.input_mode = InputMode::Normal;
            return Ok(());
        }
        if key.code == KeyCode::Enter {
            if self.account_form.is_some() {
                self.submit_account_form().await?;
            } else if self.category_form.is_some() {
                self.submit_category_form().await?;
            } else if self.transaction_form.is_some() {
                self.submit_transaction_form().await?;
            } else if self.budget_form.is_some() {
                self.submit_budget_form().await?;
            } else if self.installment_form.is_some() {
                self.submit_installment_form().await?;
            } else if self.recurring_form.is_some() {
                self.submit_recurring_form().await?;
            } else if self.transfer_form.is_some() {
                self.submit_transfer_form().await?;
            } else if self.cc_payment_form.is_some() {
                self.submit_cc_payment_form().await?;
            }
            return Ok(());
        }

        if self.account_form.is_some() {
            self.handle_account_form_key(key.code);
        } else if self.category_form.is_some() {
            self.handle_category_form_key(key.code);
        } else if self.transaction_form.is_some() {
            self.handle_transaction_form_key(key.code);
        } else if self.budget_form.is_some() {
            self.handle_budget_form_key(key.code);
        } else if self.installment_form.is_some() {
            self.handle_installment_form_key(key.code);
        } else if self.recurring_form.is_some() {
            self.handle_recurring_form_key(key.code);
        } else if self.transfer_form.is_some() {
            self.handle_transfer_form_key(key.code);
        } else if self.cc_payment_form.is_some() {
            self.handle_cc_payment_form_key(key.code);
        }
        Ok(())
    }

    // ── Dashboard key handlers ────────────────────────────────────────

    async fn handle_dashboard_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        if self.notifications.is_empty() {
            return Ok(());
        }
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.notification_selection > 0 {
                    self.notification_selection -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.notifications.len().saturating_sub(1);
                if self.notification_selection < max {
                    self.notification_selection += 1;
                }
            }
            KeyCode::Char('r') => {
                if let Some(n) = self.notifications.get(self.notification_selection) {
                    let id = n.id;
                    crate::db::notifications::mark_read(&self.pool, id).await?;
                    self.load_data().await?;
                    // Clamp selection after removal
                    if !self.notifications.is_empty() {
                        self.notification_selection = self
                            .notification_selection
                            .min(self.notifications.len() - 1);
                    } else {
                        self.notification_selection = 0;
                    }
                }
            }
            KeyCode::Char('R') => {
                crate::db::notifications::mark_all_read(&self.pool).await?;
                self.load_data().await?;
                self.notification_selection = 0;
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
            KeyCode::Left => (*idx + len - 1) % len,
            _ => (*idx + 1) % len, // Space and Right go forward
        };
    }
}
