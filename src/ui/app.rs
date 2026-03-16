use std::collections::HashMap;

use chrono::Local;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::TableState;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{
    models::{
        Account, Budget, Category, CreditCardPayment, InstallmentPurchase, RecurringTransaction,
        Transaction, Transfer,
    },
    ui::{
        components::popup::ConfirmPopup,
        screens::transactions::{TransactionFilter, TransactionForm},
    },
};

use super::screens::{
    accounts::AccountForm,
    budgets::BudgetForm,
    categories::CategoryForm,
    cc_payments::CcPaymentForm,
    installments::InstallmentForm,
    recurring::RecurringForm,
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
}

impl StatusMessage {
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: true,
        }
    }
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_error: false,
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

    pub cc_payments: Vec<CreditCardPayment>,
    pub cc_payment_table_state: TableState,
    pub cc_payment_form: Option<CcPaymentForm>,

    pub status_message: Option<StatusMessage>,
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

            cc_payments: Vec::new(),
            cc_payment_table_state: TableState::default().with_selected(0),
            cc_payment_form: None,

            status_message: None,
        }
    }

    pub fn account_name(&self, id: i32) -> &str {
        self.account_names
            .get(&id)
            .map(|s| s.as_str())
            .unwrap_or("?")
    }

    pub fn category_name(&self, id: i32) -> &str {
        self.categories
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.as_str())
            .unwrap_or("?")
    }

    pub async fn load_data(&mut self) -> anyhow::Result<()> {
        use crate::db::{accounts, budgets, categories, installments, recurring};
        use crate::models::BudgetPeriod;

        self.accounts = accounts::list_accounts(&self.pool).await?;
        self.categories = categories::list_categories(&self.pool).await?;
        self.balances = accounts::compute_all_balances(&self.pool).await?;
        self.account_names = accounts::list_all_account_names(&self.pool).await?;

        self.budgets = budgets::list_budgets(&self.pool).await?;

        let today = Local::now().date_naive();
        let (weekly_start, _) = BudgetPeriod::Weekly.date_range(today);
        let (monthly_start, _) = BudgetPeriod::Monthly.date_range(today);
        let (yearly_start, _) = BudgetPeriod::Yearly.date_range(today);
        self.budget_spent = budgets::compute_all_spending(
            &self.pool,
            weekly_start,
            monthly_start,
            yearly_start,
            today,
        )
        .await?;

        self.pending_recurring = recurring::list_pending(&self.pool, today).await?;
        self.recurring_list = recurring::list_recurring(&self.pool).await?;
        self.load_transactions().await?;

        self.installments = installments::list_installment_purchases(&self.pool).await?;
        self.transfers = crate::db::transfers::list_transfers(&self.pool, 100, 0).await?;
        self.cc_payments =
            crate::db::credit_card_payments::list_all_payments(&self.pool, 100, 0).await?;

        Ok(())
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
        // Clear status message on any key press
        self.status_message = None;

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
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char(c @ '1'..='9') => {
                let i = (c as usize) - ('1' as usize);
                if let Some(&screen) = Screen::ALL.get(i) {
                    self.screen = screen;
                }
            }
            KeyCode::Left => self.screen = self.screen.prev(),
            KeyCode::Right => self.screen = self.screen.next(),
            _ => match self.screen {
                Screen::Dashboard => {}
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
