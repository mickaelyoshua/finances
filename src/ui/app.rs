use std::collections::HashMap;

use chrono::{Datelike, Local};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::TableState;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{
    models::{Account, AccountType, Budget, Category, RecurringTransaction},
    ui::screens::accounts::{AccountField, AccountForm, AccountFormMode},
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

pub struct App {
    pub running: bool,
    pub screen: Screen,
    pub pool: PgPool,
    pub accounts: Vec<Account>,
    pub categories: Vec<Category>,
    pub balances: HashMap<i32, (Decimal, Decimal)>, // account_id -> (cheking, credit_used)
    pub budgets: Vec<Budget>,
    pub budget_spent: HashMap<i32, Decimal>, // category_id -> spent this month
    pub pending_recurring: Vec<RecurringTransaction>,
    pub account_table_state: TableState,
    pub input_mode: InputMode,
    pub account_form: Option<AccountForm>,
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
        }
    }

    pub async fn load_data(&mut self) -> anyhow::Result<()> {
        use crate::db::{accounts, budgets, categories, recurring, transactions};

        self.accounts = accounts::list_accounts(&self.pool).await?;
        self.categories = categories::list_categories(&self.pool).await?;

        self.balances.clear();
        for acc in &self.accounts {
            let checking = accounts::compute_balance(&self.pool, acc.id).await?;
            let credit = if acc.has_credit_card {
                accounts::compute_credit_used(&self.pool, acc.id).await?
            } else {
                Decimal::ZERO
            };
            self.balances.insert(acc.id, (checking, credit));
        }

        self.budgets = budgets::list_budgets(&self.pool).await?;

        self.budget_spent.clear();
        let today = Local::now().date_naive();
        let month_start = today.with_day(1).unwrap();
        for b in &self.budgets {
            let spent = transactions::sum_expenses_by_category(
                &self.pool,
                b.category_id,
                month_start,
                today,
            )
            .await?;
            self.budget_spent.insert(b.category_id, spent);
        }

        self.pending_recurring = recurring::list_pending(&self.pool, today).await?;

        Ok(())
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
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
            KeyCode::Up => {
                if self.screen == Screen::Accounts {
                    let i = self.account_table_state.selected().unwrap_or(0);
                    if i > 0 {
                        self.account_table_state.select(Some(i - 1));
                    }
                }
            }
            KeyCode::Down => {
                if self.screen == Screen::Accounts {
                    let i = self.account_table_state.selected().unwrap_or(0);
                    if i + 1 < self.accounts.len() {
                        self.account_table_state.select(Some(i + 1));
                    }
                }
            }
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
            _ => {}
        }
        Ok(())
    }

    async fn handle_editing_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        // Handle these first to avoid borrow conflict with `form`
        if key.code == KeyCode::Esc {
            self.account_form = None;
            self.input_mode = InputMode::Normal;
            return Ok(());
        }
        if key.code == KeyCode::Enter {
            self.submit_account_form().await?;
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
                            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) {
                                form.account_type = match form.account_type {
                                    AccountType::Checking => AccountType::Cash,
                                    AccountType::Cash => AccountType::Checking,
                                };
                                // Reset card flags when switching to Cash
                                if form.account_type == AccountType::Cash {
                                    form.has_credit_card = false;
                                    form.has_debit_card = false;
                                }
                                // Clamp active_field if fields became hidden
                                let max = form.visible_fields().len() - 1;
                                form.active_field = form.active_field.min(max);
                            }
                        }
                        AccountField::HasCreditCard => {
                            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) {
                                form.has_credit_card = !form.has_credit_card;
                                let max = form.visible_fields().len() - 1;
                                form.active_field = form.active_field.min(max);
                            }
                        }
                        AccountField::HasDebitCard => {
                            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) {
                                form.has_debit_card = !form.has_debit_card;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn submit_account_form(&mut self) -> anyhow::Result<()> {
        use crate::db::accounts;
        use rust_decimal::Decimal;

        let form = self.account_form.as_ref().unwrap();

        // Validade
        let name = form.name.value.trim().to_string();
        if name.is_empty() {
            self.account_form.as_mut().unwrap().error = Some("Name is required".into());
            return Ok(());
        }

        let credit_limit = if form.has_credit_card {
            match form.credit_limit.value.trim().parse::<Decimal>() {
                Ok(v) => Some(v),
                Err(_) => {
                    self.account_form.as_mut().unwrap().error = Some("Invalid credit limit".into());
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
                    self.account_form.as_mut().unwrap().error = Some("Billing day must be 1-28".into());
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
                    self.account_form.as_mut().unwrap().error = Some("Due day must be 1-28".into());
                    return Ok(());
                }
            }
        } else {
            None
        };

        match form.mode {
            AccountFormMode::Create => {
                accounts::create_account(&self.pool, &name, form.account_type, form.has_credit_card, credit_limit, billing_day, due_day, form.has_debit_card).await?;
            }
            AccountFormMode::Edit(id) => {
                accounts::update_account(&self.pool, id, &name, form.has_credit_card, credit_limit, billing_day, due_day, form.has_debit_card).await?;
            }
        }

        self.account_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }
}
