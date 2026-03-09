use std::collections::HashMap;

use chrono::{Datelike, Local};
use crossterm::event::{KeyCode, KeyEvent};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::models::{Account, Budget, Category, RecurringTransaction};

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
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char(c @ '1'..='7') => {
                let i = (c as usize) - ('1' as usize);
                self.screen = Screen::ALL[i];
            }
            KeyCode::Left => self.screen = self.screen.prev(),
            KeyCode::Right => self.screen = self.screen.next(),
            _ => {}
        }
        Ok(())
    }
}
