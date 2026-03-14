use std::collections::HashMap;

use chrono::Local;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::TableState;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{
    models::{
        Account, AccountType, Budget, Category, CategoryType, InstallmentPurchase,
        RecurringTransaction, Transaction, TransactionType,
    },
    ui::{
        components::popup::ConfirmPopup,
        screens::{
            accounts::{AccountField, AccountForm, AccountFormMode},
            budgets::{BudgetField, BudgetForm, BudgetFormMode, PERIODS},
            categories::{CategoryField, CategoryForm, CategoryFormMode},
            installments::{InstallmentField, InstallmentForm},
            recurring::{RecurringField, RecurringForm, RecurringFormMode, FREQUENCIES},
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
    DeleteBudget(i32),
    DeleteInstallment(i32),
    DeactivateRecurring(i32),
}

pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

impl StatusMessage {
    pub fn error(text: impl Into<String>) -> Self {
        Self { text: text.into(), is_error: true }
    }
    pub fn info(text: impl Into<String>) -> Self {
        Self { text: text.into(), is_error: false }
    }
}

pub struct App {
    pub running: bool,
    pub screen: Screen,
    pub pool: PgPool,
    pub accounts: Vec<Account>,
    pub categories: Vec<Category>,
    pub balances: HashMap<i32, (Decimal, Decimal)>, // account_id -> (checking, credit_used)
    pub account_names: HashMap<i32, String>,         // all accounts (incl. inactive) for lookups
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
    pub budget_table_state: TableState,
    pub budget_form: Option<BudgetForm>,
    pub installments: Vec<InstallmentPurchase>,
    pub installment_table_state: TableState,
    pub installment_form: Option<InstallmentForm>,
    pub recurring_list: Vec<RecurringTransaction>,
    pub recurring_table_state: TableState,
    pub recurring_form: Option<RecurringForm>,
    pub status_message: Option<StatusMessage>,
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
            account_names: HashMap::new(),
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
            budget_table_state: TableState::default().with_selected(0),
            budget_form: None,
            installments: Vec::new(),
            installment_table_state: TableState::default().with_selected(0),
            installment_form: None,
            recurring_list: Vec::new(),
            recurring_table_state: TableState::default().with_selected(0),
            recurring_form: None,
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
        use crate::db::{accounts, budgets, categories, installments, recurring, transactions};
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
        self.transactions = transactions::list_transactions(&self.pool, 100, 0).await?;
        self.installments = installments::list_installment_purchases(&self.pool).await?;

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
                Screen::Budgets => self.handle_budgets_key(key.code).await?,
                Screen::Installments => self.handle_installments_key(key.code).await?,
                Screen::Recurring => self.handle_recurring_key(key.code).await?,
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
                    let acc_id = acc.id;
                    let acc_name = acc.name.clone();
                    if crate::db::accounts::has_references(&self.pool, acc_id).await? {
                        self.confirm_action = Some(ConfirmAction::DeactivateAccount(acc_id));
                        self.confirm_popup = Some(ConfirmPopup::new(format!(
                            "Deactivate \"{}\"? It has existing transactions/transfers.",
                            acc_name
                        )));
                    } else {
                        self.confirm_action = Some(ConfirmAction::DeactivateAccount(acc_id));
                        self.confirm_popup =
                            Some(ConfirmPopup::new(format!("Deactivate \"{}\"?", acc_name)));
                    }
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
                        self.status_message =
                            Some(StatusMessage::error(format!("Cannot delete \"{cat_name}\": category is in use")));
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
                    self.status_message = Some(StatusMessage::error("Create an account first"));
                } else if self.categories.is_empty() {
                    self.status_message = Some(StatusMessage::error("Create a category first"));
                } else {
                    self.transaction_form = Some(TransactionForm::new_create());
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
                        self.status_message = Some(StatusMessage::error(
                            "Installment transactions are managed from the Installments screen",
                        ));
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
                        self.status_message = Some(StatusMessage::error(
                            "Installment transactions are managed from the Installments screen",
                        ));
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

    async fn handle_budgets_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up => {
                move_table_selection(&mut self.budget_table_state, self.budgets.len(), -1);
            }
            KeyCode::Down => {
                move_table_selection(&mut self.budget_table_state, self.budgets.len(), 1);
            }
            KeyCode::Char('n') => {
                let has_expense_cat = self
                    .categories
                    .iter()
                    .any(|c| c.parsed_type() == CategoryType::Expense);
                if !has_expense_cat {
                    self.status_message = Some(StatusMessage::error("Create an expense category first"));
                } else {
                    self.budget_form = Some(BudgetForm::new_create());
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('e') => {
                if let Some(b) = self
                    .budget_table_state
                    .selected()
                    .and_then(|i| self.budgets.get(i))
                {
                    let b = b.clone();
                    self.budget_form = Some(BudgetForm::new_edit(&b, &self.categories));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(b) = self
                    .budget_table_state
                    .selected()
                    .and_then(|i| self.budgets.get(i))
                {
                    let budget_id = b.id;
                    let category_name = self.category_name(b.category_id).to_string();
                    self.confirm_action = Some(ConfirmAction::DeleteBudget(budget_id));
                    self.confirm_popup = Some(ConfirmPopup::new(format!(
                        "Delete budget for \"{category_name}\"?"
                    )));
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_installments_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up => {
                move_table_selection(
                    &mut self.installment_table_state,
                    self.installments.len(),
                    -1,
                );
            }
            KeyCode::Down => {
                move_table_selection(
                    &mut self.installment_table_state,
                    self.installments.len(),
                    1,
                );
            }
            KeyCode::Char('n') => {
                let has_credit_account = self.accounts.iter().any(|a| a.has_credit_card);
                let has_expense_cat = self
                    .categories
                    .iter()
                    .any(|c| c.parsed_type() == CategoryType::Expense);
                if !has_credit_account {
                    self.status_message =
                        Some(StatusMessage::error("No account with credit card available"));
                } else if !has_expense_cat {
                    self.status_message = Some(StatusMessage::error("Create an expense category first"));
                } else {
                    self.installment_form = Some(InstallmentForm::new_create());
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(ip) = self
                    .installment_table_state
                    .selected()
                    .and_then(|i| self.installments.get(i))
                {
                    let ip_id = ip.id;
                    let ip_desc = ip.description.clone();
                    self.confirm_action = Some(ConfirmAction::DeleteInstallment(ip_id));
                    self.confirm_popup = Some(ConfirmPopup::new(format!(
                        "Delete \"{}\" and all its transactions?",
                        ip_desc
                    )));
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_recurring_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up => {
                move_table_selection(
                    &mut self.recurring_table_state,
                    self.recurring_list.len(),
                    -1,
                );
            }
            KeyCode::Down => {
                move_table_selection(
                    &mut self.recurring_table_state,
                    self.recurring_list.len(),
                    1,
                );
            }
            KeyCode::Char('n') => {
                if self.accounts.is_empty() {
                    self.status_message = Some(StatusMessage::error("Create an account first"));
                } else if self.categories.is_empty() {
                    self.status_message = Some(StatusMessage::error("Create a category first"));
                } else {
                    self.recurring_form = Some(RecurringForm::new_create());
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('e') => {
                if let Some(r) = self
                    .recurring_table_state
                    .selected()
                    .and_then(|i| self.recurring_list.get(i))
                {
                    let r = r.clone();
                    self.recurring_form = Some(RecurringForm::new_edit(
                        &r,
                        &self.accounts,
                        &self.categories,
                    ));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(r) = self
                    .recurring_table_state
                    .selected()
                    .and_then(|i| self.recurring_list.get(i))
                {
                    let r_id = r.id;
                    let r_desc = r.description.clone();
                    self.confirm_action = Some(ConfirmAction::DeactivateRecurring(r_id));
                    self.confirm_popup = Some(ConfirmPopup::new(format!(
                        "Deactivate \"{}\"?",
                        r_desc
                    )));
                }
            }
            KeyCode::Char('c') => {
                self.confirm_recurring().await?;
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
            self.budget_form = None;
            self.installment_form = None;
            self.recurring_form = None;
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
                        cycle_index(&mut form.account_idx, self.accounts.len(), code);
                        form.payment_method_idx = 0;
                    }
                }
                TransactionField::PaymentMethod => {
                    if is_toggle_key(code) {
                        let len = self
                            .accounts
                            .get(form.account_idx)
                            .map(|a| a.allowed_payment_methods().len())
                            .unwrap_or(0);
                        cycle_index(&mut form.payment_method_idx, len, code);
                    }
                }
                TransactionField::Category => {
                    if is_toggle_key(code) {
                        let len = self
                            .categories
                            .iter()
                            .filter(|c| c.parsed_type() == form.transaction_type.category_type())
                            .count();
                        cycle_index(&mut form.category_idx, len, code);
                    }
                }
            },
        }
    }

    fn handle_budget_form_key(&mut self, code: KeyCode) {
        let form = self.budget_form.as_mut().unwrap();
        let is_edit = matches!(form.mode, BudgetFormMode::Edit(_));

        match code {
            KeyCode::Tab | KeyCode::Down => {
                if is_edit {
                    // Only Amount (index 1) is editable in edit mode
                    form.active_field = 1;
                } else if form.active_field < BudgetField::ALL.len() - 1 {
                    form.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if is_edit {
                    form.active_field = 1;
                } else if form.active_field > 0 {
                    form.active_field -= 1;
                }
            }
            _ => match form.active_field_id() {
                BudgetField::Amount => form.amount.handle_key(code),
                BudgetField::Category if !is_edit => {
                    if is_toggle_key(code) {
                        let len = self
                            .categories
                            .iter()
                            .filter(|c| c.parsed_type() == CategoryType::Expense)
                            .count();
                        cycle_index(&mut form.category_idx, len, code);
                    }
                }
                BudgetField::Period if !is_edit => {
                    if is_toggle_key(code) {
                        cycle_index(&mut form.period_idx, PERIODS.len(), code);
                    }
                }
                _ => {} // locked fields in edit mode
            },
        }
    }

    fn handle_installment_form_key(&mut self, code: KeyCode) {
        let form = self.installment_form.as_mut().unwrap();
        match code {
            KeyCode::Tab | KeyCode::Down => {
                if form.active_field < InstallmentField::ALL.len() - 1 {
                    form.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if form.active_field > 0 {
                    form.active_field -= 1;
                }
            }
            _ => match form.active_field_id() {
                InstallmentField::Description => form.description.handle_key(code),
                InstallmentField::TotalAmount => form.total_amount.handle_key(code),
                InstallmentField::InstallmentCount => form.installment_count.handle_key(code),
                InstallmentField::FirstDate => form.first_date.handle_key(code),
                InstallmentField::Account => {
                    if is_toggle_key(code) {
                        let len = self.accounts.iter().filter(|a| a.has_credit_card).count();
                        cycle_index(&mut form.account_idx, len, code);
                    }
                }
                InstallmentField::Category => {
                    if is_toggle_key(code) {
                        let len = self
                            .categories
                            .iter()
                            .filter(|c| c.parsed_type() == CategoryType::Expense)
                            .count();
                        cycle_index(&mut form.category_idx, len, code);
                    }
                }
            },
        }
    }

    fn handle_recurring_form_key(&mut self, code: KeyCode) {
        let form = self.recurring_form.as_mut().unwrap();
        match code {
            KeyCode::Tab | KeyCode::Down => {
                if form.active_field < RecurringField::ALL.len() - 1 {
                    form.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if form.active_field > 0 {
                    form.active_field -= 1;
                }
            }
            _ => match form.active_field_id() {
                RecurringField::Description => form.description.handle_key(code),
                RecurringField::Amount => form.amount.handle_key(code),
                RecurringField::NextDue => form.next_due.handle_key(code),
                RecurringField::TransactionType => {
                    if is_toggle_key(code) {
                        form.transaction_type = match form.transaction_type {
                            TransactionType::Expense => TransactionType::Income,
                            TransactionType::Income => TransactionType::Expense,
                        };
                        form.category_idx = 0;
                    }
                }
                RecurringField::Account => {
                    if is_toggle_key(code) {
                        cycle_index(&mut form.account_idx, self.accounts.len(), code);
                        form.payment_method_idx = 0;
                    }
                }
                RecurringField::PaymentMethod => {
                    if is_toggle_key(code) {
                        let len = self
                            .accounts
                            .get(form.account_idx)
                            .map(|a| a.allowed_payment_methods().len())
                            .unwrap_or(0);
                        cycle_index(&mut form.payment_method_idx, len, code);
                    }
                }
                RecurringField::Category => {
                    if is_toggle_key(code) {
                        let len = self
                            .categories
                            .iter()
                            .filter(|c| c.parsed_type() == form.transaction_type.category_type())
                            .count();
                        cycle_index(&mut form.category_idx, len, code);
                    }
                }
                RecurringField::Frequency => {
                    if is_toggle_key(code) {
                        cycle_index(&mut form.frequency_idx, FREQUENCIES.len(), code);
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

        // Extract mode info before dropping the shared borrow
        let edit_id = match form.mode {
            AccountFormMode::Create => None,
            AccountFormMode::Edit(id) => Some(id),
        };

        if let Some(id) = edit_id {
            // Guard: block breaking changes when the account has transactions
            if let Some(old) = self.accounts.iter().find(|a| a.id == id) {
                let type_changed = old.parsed_type() != validated.account_type;
                let credit_removed = old.has_credit_card && !validated.has_credit_card;
                let debit_removed = old.has_debit_card && !validated.has_debit_card;

                if type_changed || credit_removed || debit_removed {
                    let used = accounts::used_payment_methods(&self.pool, id).await?;

                    if type_changed && !used.is_empty() {
                        self.account_form.as_mut().unwrap().error =
                            Some("Cannot change account type: account has transactions".into());
                        return Ok(());
                    }
                    if credit_removed && used.iter().any(|m| m == "credit") {
                        self.account_form.as_mut().unwrap().error =
                            Some("Cannot disable credit card: account has credit transactions".into());
                        return Ok(());
                    }
                    if debit_removed && used.iter().any(|m| m == "debit") {
                        self.account_form.as_mut().unwrap().error =
                            Some("Cannot disable debit card: account has debit transactions".into());
                        return Ok(());
                    }
                }
            }

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
        } else {
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

        // Extract mode info before dropping the shared borrow
        let edit_id = match form.mode {
            CategoryFormMode::Create => None,
            CategoryFormMode::Edit(id) => Some(id),
        };

        if let Some(id) = edit_id {
            // Guard: block type change if category is referenced
            if let Some(old) = self.categories.iter().find(|c| c.id == id) {
                if old.parsed_type() != validated.category_type
                    && categories::has_references(&self.pool, id).await?
                {
                    self.category_form.as_mut().unwrap().error =
                        Some("Cannot change type: category is referenced by transactions or budgets".into());
                    return Ok(());
                }
            }

            categories::update_category(
                &self.pool,
                id,
                &validated.name,
                validated.category_type,
            )
            .await?;
        } else {
            categories::create_category(&self.pool, &validated.name, validated.category_type)
                .await?;
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

    async fn submit_budget_form(&mut self) -> anyhow::Result<()> {
        use crate::db::budgets;

        let form = self.budget_form.as_ref().unwrap();
        let validated = match form.validate(&self.categories) {
            Ok(v) => v,
            Err(e) => {
                self.budget_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        let is_edit = match form.mode {
            BudgetFormMode::Create => false,
            BudgetFormMode::Edit(_) => true,
        };

        if is_edit {
            let id = match form.mode {
                BudgetFormMode::Edit(id) => id,
                _ => unreachable!(),
            };
            budgets::update_budget(&self.pool, id, validated.amount).await?;
        } else {
            match budgets::create_budget(
                &self.pool,
                validated.category_id,
                validated.amount,
                validated.period,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    if e.as_database_error()
                        .is_some_and(|db_err| db_err.is_unique_violation())
                    {
                        self.budget_form.as_mut().unwrap().error = Some(
                            "A budget for this category and period already exists".into(),
                        );
                        return Ok(());
                    }
                    return Err(e.into());
                }
            }
        }

        self.budget_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    async fn execute_delete_budget(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::budgets::delete_budget(&self.pool, id).await?;
        self.load_data().await?;
        clamp_selection(&mut self.budget_table_state, self.budgets.len());
        Ok(())
    }

    async fn submit_installment_form(&mut self) -> anyhow::Result<()> {
        use crate::db::installments;

        let form = self.installment_form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts, &self.categories) {
            Ok(v) => v,
            Err(e) => {
                self.installment_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        installments::create_installment_purchase(
            &self.pool,
            validated.total_amount,
            validated.installment_count,
            &validated.description,
            validated.category_id,
            validated.account_id,
            validated.first_date,
        )
        .await?;

        self.installment_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    async fn execute_delete_installment(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::installments::delete_installment_purchase(&self.pool, id).await?;
        self.load_data().await?;
        clamp_selection(&mut self.installment_table_state, self.installments.len());
        Ok(())
    }

    async fn submit_recurring_form(&mut self) -> anyhow::Result<()> {
        use crate::db::recurring;

        let form = self.recurring_form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts, &self.categories) {
            Ok(v) => v,
            Err(e) => {
                self.recurring_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        match form.mode {
            RecurringFormMode::Create => {
                recurring::create_recurring(
                    &self.pool,
                    validated.amount,
                    &validated.description,
                    validated.category_id,
                    validated.account_id,
                    validated.transaction_type,
                    validated.payment_method,
                    validated.frequency,
                    validated.next_due,
                )
                .await?;
            }
            RecurringFormMode::Edit(id) => {
                recurring::update_recurring(
                    &self.pool,
                    id,
                    validated.amount,
                    &validated.description,
                    validated.category_id,
                    validated.account_id,
                    validated.transaction_type,
                    validated.payment_method,
                    validated.frequency,
                    validated.next_due,
                )
                .await?;
            }
        }

        self.recurring_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    async fn confirm_recurring(&mut self) -> anyhow::Result<()> {
        use crate::db::recurring;

        let today = Local::now().date_naive();

        let r = match self
            .recurring_table_state
            .selected()
            .and_then(|i| self.recurring_list.get(i))
        {
            Some(r) => r.clone(),
            None => return Ok(()),
        };

        if r.next_due > today {
            self.status_message = Some(StatusMessage::error(format!(
                "Not due yet (next due: {})",
                r.next_due.format("%d-%m-%Y")
            )));
            return Ok(());
        }

        let new_next_due = recurring::compute_next_due(r.next_due, r.parsed_frequency());

        // Atomic: create the transaction AND advance the due date in one transaction
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(r.amount)
        .bind(&r.description)
        .bind(r.category_id)
        .bind(r.account_id)
        .bind(r.parsed_type().as_str())
        .bind(r.parsed_payment_method().as_str())
        .bind(r.next_due)
        .execute(&mut *tx)
        .await?;

        sqlx::query("UPDATE recurring_transactions SET next_due = $2 WHERE id = $1")
            .bind(r.id)
            .bind(new_next_due)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        self.load_data().await?;
        self.status_message = Some(StatusMessage::info(format!(
            "Confirmed \"{}\" — next due: {}",
            r.description,
            new_next_due.format("%d-%m-%Y")
        )));
        Ok(())
    }

    async fn execute_deactivate_recurring(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::recurring::deactivate_recurring(&self.pool, id).await?;
        self.load_data().await?;
        clamp_selection(&mut self.recurring_table_state, self.recurring_list.len());
        Ok(())
    }
}

fn move_table_selection(state: &mut TableState, len: usize, delta: isize) {
    let i = state.selected().unwrap_or(0);
    let new = (i as isize + delta).clamp(0, len.saturating_sub(1) as isize) as usize;
    state.select(Some(new));
}

fn clamp_selection(state: &mut TableState, len: usize) {
    if len == 0 {
        state.select(None);
    } else {
        let max = len - 1;
        let i = state.selected().unwrap_or(0);
        state.select(Some(i.min(max)));
    }
}

fn is_toggle_key(code: KeyCode) -> bool {
    matches!(code, KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right)
}

fn cycle_index(idx: &mut usize, len: usize, code: KeyCode) {
    if len > 0 {
        *idx = match code {
            KeyCode::Left => (*idx + len - 1) % len,
            _ => (*idx + 1) % len, // Space and Right go forward
        };
    }
}
