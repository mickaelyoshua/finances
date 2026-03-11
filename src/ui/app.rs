use std::collections::HashMap;

use chrono::Local;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::TableState;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{
    models::{Account, AccountType, Budget, Category, CategoryType, RecurringTransaction},
    ui::{
        components::popup::ConfirmPopup,
        screens::{
            accounts::{AccountField, AccountForm, AccountFormMode},
            categories::{CategoryField, CategoryForm, CategoryFormMode},
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
                transactions::sum_expenses_by_category(&self.pool, b.category_id, from, to)
                    .await?;
            self.budget_spent.insert(b.id, spent);
        }

        self.pending_recurring = recurring::list_pending(&self.pool, today).await?;

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

    async fn handle_normal_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char(c @ '1'..='7') => {
                let i = (c as usize) - ('1' as usize);
                self.screen = Screen::ALL[i];
            }
            KeyCode::Left => self.screen = self.screen.prev(),
            KeyCode::Right => self.screen = self.screen.next(),
            KeyCode::Up => match self.screen {
                Screen::Accounts => {
                    move_table_selection(&mut self.account_table_state, self.accounts.len(), -1);
                }
                Screen::Categories => {
                    move_table_selection(&mut self.category_table_state, self.categories.len(), -1);
                }
                _ => {}
            },
            KeyCode::Down => match self.screen {
                Screen::Accounts => {
                    move_table_selection(&mut self.account_table_state, self.accounts.len(), 1);
                }
                Screen::Categories => {
                    move_table_selection(&mut self.category_table_state, self.categories.len(), 1);
                }
                _ => {}
            },
            KeyCode::Char('n') if self.screen == Screen::Accounts => {
                self.account_form = Some(AccountForm::new_create());
                self.input_mode = InputMode::Editing;
            }
            KeyCode::Char('e') if self.screen == Screen::Accounts => {
                if let Some(acc) = self
                    .account_table_state
                    .selected()
                    .and_then(|i| self.accounts.get(i))
                {
                    self.account_form = Some(AccountForm::new_edit(acc));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') if self.screen == Screen::Accounts => {
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
            KeyCode::Char('n') if self.screen == Screen::Categories => {
                self.category_form = Some(CategoryForm::new_create());
                self.input_mode = InputMode::Editing;
            }
            KeyCode::Char('e') if self.screen == Screen::Categories => {
                if let Some(cat) = self
                    .category_table_state
                    .selected()
                    .and_then(|i| self.categories.get(i))
                {
                    self.category_form = Some(CategoryForm::new_edit(cat));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') if self.screen == Screen::Categories => {
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

    async fn handle_editing_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        // Handle these first to avoid borrow conflict with `form`
        if key.code == KeyCode::Esc {
            self.account_form = None;
            self.category_form = None;
            self.input_mode = InputMode::Normal;
            return Ok(());
        }
        if key.code == KeyCode::Enter {
            if self.account_form.is_some() {
                self.submit_account_form().await?;
            } else if self.category_form.is_some() {
                self.submit_category_form().await?;
            }
            return Ok(());
        }

        if let Some(form) = &mut self.account_form {
            match key.code {
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
                _ => {
                    match form.active_field_id() {
                        AccountField::Name => form.name.handle_key(key.code),
                        AccountField::CreditLimit => form.credit_limit.handle_key(key.code),
                        AccountField::BillingDay => form.billing_day.handle_key(key.code),
                        AccountField::DueDay => form.due_day.handle_key(key.code),
                        // Toggle fields
                        AccountField::AccountType => {
                            if is_toggle_key(key.code) {
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
                            if is_toggle_key(key.code) {
                                form.has_credit_card = !form.has_credit_card;
                                let max = form.visible_fields().len() - 1;
                                form.active_field = form.active_field.min(max);
                            }
                        }
                        AccountField::HasDebitCard => {
                            if is_toggle_key(key.code) {
                                form.has_debit_card = !form.has_debit_card;
                            }
                        }
                    }
                }
            }
        } else if let Some(form) = &mut self.category_form {
            match key.code {
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
                    CategoryField::Name => form.name.handle_key(key.code),
                    CategoryField::CategoryType => {
                        if is_toggle_key(key.code) {
                            form.category_type = match form.category_type {
                                CategoryType::Expense => CategoryType::Income,
                                CategoryType::Income => CategoryType::Expense,
                            };
                        }
                    }
                },
            }
        }
        Ok(())
    }

    async fn submit_account_form(&mut self) -> anyhow::Result<()> {
        use crate::db::accounts;
        use rust_decimal::Decimal;

        let form = self.account_form.as_ref().unwrap();

        // Validate
        let name = form.name.value.trim().to_string();
        if name.is_empty() {
            self.account_form.as_mut().unwrap().error = Some("Name is required".into());
            return Ok(());
        }

        let credit_limit = if form.has_credit_card {
            match form.credit_limit.value.trim().parse::<Decimal>() {
                Ok(v) if v > Decimal::ZERO => Some(v),
                Ok(_) => {
                    self.account_form.as_mut().unwrap().error =
                        Some("Credit limit must be positive".into());
                    return Ok(());
                }
                Err(_) => {
                    self.account_form.as_mut().unwrap().error =
                        Some("Invalid credit limit".into());
                    return Ok(());
                }
            }
        } else {
            None
        };

        let billing_day = if form.has_credit_card {
            match form.billing_day.value.trim().parse::<i16>() {
                Ok(v) if (1..=28).contains(&v) => Some(v),
                _ => {
                    self.account_form.as_mut().unwrap().error =
                        Some("Billing day must be 1-28".into());
                    return Ok(());
                }
            }
        } else {
            None
        };

        let due_day = if form.has_credit_card {
            match form.due_day.value.trim().parse::<i16>() {
                Ok(v) if (1..=28).contains(&v) => Some(v),
                _ => {
                    self.account_form.as_mut().unwrap().error =
                        Some("Due day must be 1-28".into());
                    return Ok(());
                }
            }
        } else {
            None
        };

        match form.mode {
            AccountFormMode::Create => {
                accounts::create_account(
                    &self.pool,
                    &name,
                    form.account_type,
                    form.has_credit_card,
                    credit_limit,
                    billing_day,
                    due_day,
                    form.has_debit_card,
                )
                .await?;
            }
            AccountFormMode::Edit(id) => {
                accounts::update_account(
                    &self.pool,
                    id,
                    &name,
                    form.account_type,
                    form.has_credit_card,
                    credit_limit,
                    billing_day,
                    due_day,
                    form.has_debit_card,
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

        let name = form.name.value.trim().to_string();
        if name.is_empty() {
            self.category_form.as_mut().unwrap().error = Some("Name is required".into());
            return Ok(());
        }

        match form.mode {
            CategoryFormMode::Create => {
                categories::create_category(&self.pool, &name, form.category_type).await?;
            }
            CategoryFormMode::Edit(id) => {
                categories::update_category(&self.pool, id, &name, form.category_type).await?;
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
