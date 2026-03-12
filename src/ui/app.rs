use std::collections::HashMap;

use chrono::Local;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::TableState;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{
    models::{
        Account, AccountType, Budget, Category, CategoryType, RecurringTransaction, Transaction,
        TransactionType,
    },
    ui::{
        components::popup::ConfirmPopup,
        screens::{
            accounts::{AccountField, AccountForm, AccountFormMode},
            categories::{CategoryField, CategoryForm, CategoryFormMode},
            transactions::{TransactionField, TransactionForm, TransactionFormMode},
        },
    },
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
}

impl Screen {
    pub const ALL: [Self; 7] = [
        Self::Dashboard,
        Self::Transactions,
        Self::Accounts,
        Self::Budgets,
        Self::Categories,
        Self::Installments,
        Self::Recurring,
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
}

pub enum ConfirmAction {
    DeactivateAccount(i32),
    DeleteCategory(i32),
    DeleteTransaction(i32),
}

pub struct App {
    pub running: bool,
    pub screen: Screen,
    pub pool: PgPool,
    pub accounts: Vec<Account>,
    pub categories: Vec<Category>,
    pub balances: HashMap<i32, (Decimal, Decimal)>, // account_id -> (checking, credit_used)
    pub budgets: Vec<Budget>,
    pub budget_spent: HashMap<i32, Decimal>, // budget_id -> spent in current period
    pub pending_recurring: Vec<RecurringTransaction>,
    pub account_table_state: TableState,
    pub input_mode: InputMode,
    pub account_form: Option<AccountForm>,
    pub confirm_popup: Option<ConfirmPopup>,
    pub confirm_action: Option<ConfirmAction>,
    pub category_table_state: TableState,
    pub category_form: Option<CategoryForm>,
    pub transactions: Vec<Transaction>,
    pub transaction_table_state: TableState,
    pub transaction_form: Option<TransactionForm>,
    pub status_error: Option<String>,
}

impl App {
    pub fn new(pool: PgPool) -> Self {
        Self {
            running: true,
            screen: Screen::Dashboard,
            pool,
            accounts: Vec::new(),
            categories: Vec::new(),
            balances: HashMap::new(),
            budgets: Vec::new(),
            budget_spent: HashMap::new(),
            pending_recurring: Vec::new(),
            account_table_state: TableState::default().with_selected(0),
            input_mode: InputMode::Normal,
            account_form: None,
            confirm_popup: None,
            confirm_action: None,
            category_table_state: TableState::default().with_selected(0),
            category_form: None,
            transactions: Vec::new(),
            transaction_table_state: TableState::default().with_selected(0),
            transaction_form: None,
            status_error: None,
        }
    }

    pub async fn load_data(&mut self) -> anyhow::Result<()> {
        use crate::db::{accounts, budgets, categories, recurring, transactions};

        self.accounts = accounts::list_accounts(&self.pool).await?;
        self.categories = categories::list_categories(&self.pool).await?;
        self.balances = accounts::compute_all_balances(&self.pool).await?;

        self.budgets = budgets::list_budgets(&self.pool).await?;

        self.budget_spent.clear();
        let today = Local::now().date_naive();
        for b in &self.budgets {
            let (from, to) = b.parsed_period().date_range(today);
            let spent =
                transactions::sum_expenses_by_category(&self.pool, b.category_id, from, to).await?;
            self.budget_spent.insert(b.id, spent);
        }

        self.pending_recurring = recurring::list_pending(&self.pool, today).await?;
        self.transactions = transactions::list_transactions(&self.pool, 100, 0).await?;

        Ok(())
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        // Clear status error on any key press
        self.status_error = None;

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
        }
    }

    // ── Normal-mode dispatch ──────────────────────────────────────────

    async fn handle_normal_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char(c @ '1'..='7') => {
                let i = (c as usize) - ('1' as usize);
                self.screen = Screen::ALL[i];
            }
            KeyCode::Left => self.screen = self.screen.prev(),
            KeyCode::Right => self.screen = self.screen.next(),
            _ => match self.screen {
                Screen::Accounts => self.handle_accounts_key(key.code).await?,
                Screen::Categories => self.handle_categories_key(key.code).await?,
                Screen::Transactions => self.handle_transactions_key(key.code).await?,
                _ => {}
            },
        }
        Ok(())
    }

    async fn handle_accounts_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up => {
                move_table_selection(&mut self.account_table_state, self.accounts.len(), -1);
            }
            KeyCode::Down => {
                move_table_selection(&mut self.account_table_state, self.accounts.len(), 1);
            }
            KeyCode::Char('n') => {
                self.account_form = Some(AccountForm::new_create());
                self.input_mode = InputMode::Editing;
            }
            KeyCode::Char('e') => {
                if let Some(acc) = self
                    .account_table_state
                    .selected()
                    .and_then(|i| self.accounts.get(i))
                {
                    self.account_form = Some(AccountForm::new_edit(acc));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(acc) = self
                    .account_table_state
                    .selected()
                    .and_then(|i| self.accounts.get(i))
                {
                    self.confirm_action = Some(ConfirmAction::DeactivateAccount(acc.id));
                    self.confirm_popup =
                        Some(ConfirmPopup::new(format!("Deactivate \"{}\"?", acc.name)));
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_categories_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up => {
                move_table_selection(
                    &mut self.category_table_state,
                    self.categories.len(),
                    -1,
                );
            }
            KeyCode::Down => {
                move_table_selection(
                    &mut self.category_table_state,
                    self.categories.len(),
                    1,
                );
            }
            KeyCode::Char('n') => {
                self.category_form = Some(CategoryForm::new_create());
                self.input_mode = InputMode::Editing;
            }
            KeyCode::Char('e') => {
                if let Some(cat) = self
                    .category_table_state
                    .selected()
                    .and_then(|i| self.categories.get(i))
                {
                    self.category_form = Some(CategoryForm::new_edit(cat));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(cat) = self
                    .category_table_state
                    .selected()
                    .and_then(|i| self.categories.get(i))
                {
                    let cat_id = cat.id;
                    let cat_name = cat.name.clone();
                    if crate::db::categories::has_references(&self.pool, cat_id).await? {
                        self.status_error =
                            Some(format!("Cannot delete \"{cat_name}\": category is in use"));
                    } else {
                        self.confirm_action = Some(ConfirmAction::DeleteCategory(cat_id));
                        self.confirm_popup =
                            Some(ConfirmPopup::new(format!("Delete \"{cat_name}\"?")));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_transactions_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up => {
                move_table_selection(
                    &mut self.transaction_table_state,
                    self.transactions.len(),
                    -1,
                );
            }
            KeyCode::Down => {
                move_table_selection(
                    &mut self.transaction_table_state,
                    self.transactions.len(),
                    1,
                );
            }
            KeyCode::Char('n') => {
                if self.accounts.is_empty() {
                    self.status_error = Some("Create an account first".into());
                } else if self.categories.is_empty() {
                    self.status_error = Some("Create a category first".into());
                } else {
                    self.transaction_form = Some(TransactionForm::new_create(
                        &self.accounts,
                        &self.categories,
                    ));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('e') => {
                if let Some(txn) = self
                    .transaction_table_state
                    .selected()
                    .and_then(|i| self.transactions.get(i))
                {
                    if txn.installment_purchase_id.is_some() {
                        self.status_error = Some(
                            "Installment transactions are managed from the Installments screen"
                                .into(),
                        );
                    } else {
                        let txn = txn.clone();
                        self.transaction_form = Some(TransactionForm::new_edit(
                            &txn,
                            &self.accounts,
                            &self.categories,
                        ));
                        self.input_mode = InputMode::Editing;
                    }
                }
            }
            KeyCode::Char('d') => {
                if let Some(txn) = self
                    .transaction_table_state
                    .selected()
                    .and_then(|i| self.transactions.get(i))
                {
                    if txn.installment_purchase_id.is_some() {
                        self.status_error = Some(
                            "Installment transactions are managed from the Installments screen"
                                .into(),
                        );
                    } else {
                        let txn_id = txn.id;
                        let txn_desc = txn.description.clone();
                        self.confirm_action = Some(ConfirmAction::DeleteTransaction(txn_id));
                        self.confirm_popup =
                            Some(ConfirmPopup::new(format!("Delete \"{txn_desc}\"?")));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── Editing-mode dispatch ─────────────────────────────────────────

    async fn handle_editing_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if key.code == KeyCode::Esc {
            self.account_form = None;
            self.category_form = None;
            self.transaction_form = None;
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
            }
            return Ok(());
        }

        if self.account_form.is_some() {
            self.handle_account_form_key(key.code);
        } else if self.category_form.is_some() {
            self.handle_category_form_key(key.code);
        } else if self.transaction_form.is_some() {
            self.handle_transaction_form_key(key.code);
        }
        Ok(())
    }

    fn handle_account_form_key(&mut self, code: KeyCode) {
        let form = self.account_form.as_mut().unwrap();
        match code {
            KeyCode::Tab | KeyCode::Down => {
                let max = form.visible_fields().len() - 1;
                if form.active_field < max {
                    form.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if form.active_field > 0 {
                    form.active_field -= 1;
                }
            }
            _ => match form.active_field_id() {
                AccountField::Name => form.name.handle_key(code),
                AccountField::CreditLimit => form.credit_limit.handle_key(code),
                AccountField::BillingDay => form.billing_day.handle_key(code),
                AccountField::DueDay => form.due_day.handle_key(code),
                AccountField::AccountType => {
                    if is_toggle_key(code) {
                        form.account_type = match form.account_type {
                            AccountType::Checking => AccountType::Cash,
                            AccountType::Cash => AccountType::Checking,
                        };
                        if form.account_type == AccountType::Cash {
                            form.has_credit_card = false;
                            form.has_debit_card = false;
                        }
                        let max = form.visible_fields().len() - 1;
                        form.active_field = form.active_field.min(max);
                    }
                }
                AccountField::HasCreditCard => {
                    if is_toggle_key(code) {
                        form.has_credit_card = !form.has_credit_card;
                        let max = form.visible_fields().len() - 1;
                        form.active_field = form.active_field.min(max);
                    }
                }
                AccountField::HasDebitCard => {
                    if is_toggle_key(code) {
                        form.has_debit_card = !form.has_debit_card;
                    }
                }
            },
        }
    }

    fn handle_category_form_key(&mut self, code: KeyCode) {
        let form = self.category_form.as_mut().unwrap();
        match code {
            KeyCode::Tab | KeyCode::Down => {
                if form.active_field < CategoryField::ALL.len() - 1 {
                    form.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if form.active_field > 0 {
                    form.active_field -= 1;
                }
            }
            _ => match form.active_field_id() {
                CategoryField::Name => form.name.handle_key(code),
                CategoryField::CategoryType => {
                    if is_toggle_key(code) {
                        form.category_type = match form.category_type {
                            CategoryType::Expense => CategoryType::Income,
                            CategoryType::Income => CategoryType::Expense,
                        };
                    }
                }
            },
        }
    }

    fn handle_transaction_form_key(&mut self, code: KeyCode) {
        let form = self.transaction_form.as_mut().unwrap();
        match code {
            KeyCode::Tab | KeyCode::Down => {
                if form.active_field < TransactionField::ALL.len() - 1 {
                    form.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if form.active_field > 0 {
                    form.active_field -= 1;
                }
            }
            _ => match form.active_field_id() {
                TransactionField::Date => form.date.handle_key(code),
                TransactionField::Description => form.description.handle_key(code),
                TransactionField::Amount => form.amount.handle_key(code),
                TransactionField::TransactionType => {
                    if is_toggle_key(code) {
                        form.transaction_type = match form.transaction_type {
                            TransactionType::Expense => TransactionType::Income,
                            TransactionType::Income => TransactionType::Expense,
                        };
                        form.category_idx = 0;
                    }
                }
                TransactionField::Account => {
                    if is_toggle_key(code) {
                        let len = self.accounts.len();
                        if len > 0 {
                            form.account_idx = match code {
                                KeyCode::Left | KeyCode::Char(' ') => {
                                    (form.account_idx + len - 1) % len
                                }
                                _ => (form.account_idx + 1) % len,
                            };
                            form.payment_method_idx = 0;
                        }
                    }
                }
                TransactionField::PaymentMethod => {
                    if is_toggle_key(code) {
                        let methods = self
                            .accounts
                            .get(form.account_idx)
                            .map(|a| a.allowed_payment_methods())
                            .unwrap_or_default();
                        let len = methods.len();
                        if len > 0 {
                            form.payment_method_idx = match code {
                                KeyCode::Left | KeyCode::Char(' ') => {
                                    (form.payment_method_idx + len - 1) % len
                                }
                                _ => (form.payment_method_idx + 1) % len,
                            };
                        }
                    }
                }
                TransactionField::Category => {
                    if is_toggle_key(code) {
                        let len = self
                            .categories
                            .iter()
                            .filter(|c| {
                                c.parsed_type()
                                    == crate::ui::screens::transactions::category_type_for(
                                        form.transaction_type,
                                    )
                            })
                            .count();
                        if len > 0 {
                            form.category_idx = match code {
                                KeyCode::Left | KeyCode::Char(' ') => {
                                    (form.category_idx + len - 1) % len
                                }
                                _ => (form.category_idx + 1) % len,
                            };
                        }
                    }
                }
            },
        }
    }

    // ── Form submission ───────────────────────────────────────────────

    async fn submit_account_form(&mut self) -> anyhow::Result<()> {
        use crate::db::accounts;

        let form = self.account_form.as_ref().unwrap();
        let validated = match form.validate() {
            Ok(v) => v,
            Err(e) => {
                self.account_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        match form.mode {
            AccountFormMode::Create => {
                accounts::create_account(
                    &self.pool,
                    &validated.name,
                    validated.account_type,
                    validated.has_credit_card,
                    validated.credit_limit,
                    validated.billing_day,
                    validated.due_day,
                    validated.has_debit_card,
                )
                .await?;
            }
            AccountFormMode::Edit(id) => {
                accounts::update_account(
                    &self.pool,
                    id,
                    &validated.name,
                    validated.account_type,
                    validated.has_credit_card,
                    validated.credit_limit,
                    validated.billing_day,
                    validated.due_day,
                    validated.has_debit_card,
                )
                .await?;
            }
        }

        self.account_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    async fn execute_deactivate(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::accounts::deactivate_account(&self.pool, id).await?;
        self.load_data().await?;
        clamp_selection(&mut self.account_table_state, self.accounts.len());
        Ok(())
    }

    async fn submit_category_form(&mut self) -> anyhow::Result<()> {
        use crate::db::categories;

        let form = self.category_form.as_ref().unwrap();
        let validated = match form.validate() {
            Ok(v) => v,
            Err(e) => {
                self.category_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        match form.mode {
            CategoryFormMode::Create => {
                categories::create_category(&self.pool, &validated.name, validated.category_type)
                    .await?;
            }
            CategoryFormMode::Edit(id) => {
                categories::update_category(
                    &self.pool,
                    id,
                    &validated.name,
                    validated.category_type,
                )
                .await?;
            }
        }

        self.category_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    async fn execute_delete_category(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::categories::delete_category(&self.pool, id).await?;
        self.load_data().await?;
        clamp_selection(&mut self.category_table_state, self.categories.len());
        Ok(())
    }

    async fn submit_transaction_form(&mut self) -> anyhow::Result<()> {
        use crate::db::transactions;

        let form = self.transaction_form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts, &self.categories) {
            Ok(v) => v,
            Err(e) => {
                self.transaction_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        match form.mode {
            TransactionFormMode::Create => {
                transactions::create_transaction(
                    &self.pool,
                    validated.amount,
                    &validated.description,
                    validated.category_id,
                    validated.account_id,
                    validated.transaction_type,
                    validated.payment_method,
                    validated.date,
                )
                .await?;
            }
            TransactionFormMode::Edit(id) => {
                transactions::update_transaction(
                    &self.pool,
                    id,
                    validated.amount,
                    &validated.description,
                    validated.category_id,
                    validated.account_id,
                    validated.transaction_type,
                    validated.payment_method,
                    validated.date,
                )
                .await?;
            }
        }

        self.transaction_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    async fn execute_delete_transaction(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::transactions::delete_transaction(&self.pool, id).await?;
        self.load_data().await?;
        clamp_selection(&mut self.transaction_table_state, self.transactions.len());
        Ok(())
    }
}

fn move_table_selection(state: &mut TableState, len: usize, delta: isize) {
    let i = state.selected().unwrap_or(0);
    let new = (i as isize + delta).clamp(0, len.saturating_sub(1) as isize) as usize;
    state.select(Some(new));
}

fn clamp_selection(state: &mut TableState, len: usize) {
    let max = len.saturating_sub(1);
    let i = state.selected().unwrap_or(0);
    state.select(Some(i.min(max)));
}

fn is_toggle_key(code: KeyCode) -> bool {
    matches!(code, KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right)
}
