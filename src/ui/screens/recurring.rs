use chrono::{Local, NaiveDate};
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use rust_decimal::Decimal;
use tracing::info;

use crate::{
    models::{Account, Category, Frequency, PaymentMethod, RecurringTransaction, TransactionType},
    ui::{
        App,
        app::{
            ConfirmAction, InputMode, StatusMessage, clamp_selection, cycle_index, is_toggle_key,
            move_table_selection,
        },
        components::{
            format::{format_brl, parse_positive_amount},
            input::InputField,
            popup::ConfirmPopup,
            toggle::{push_form_error, render_selector, render_toggle},
        },
    },
};

pub enum RecurringFormMode {
    Create,
    Edit(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurringField {
    Description,
    Amount,
    TransactionType,
    Account,
    PaymentMethod,
    Category,
    Frequency,
    NextDue,
}

impl RecurringField {
    pub const ALL: [Self; 8] = [
        Self::Description,
        Self::Amount,
        Self::TransactionType,
        Self::Account,
        Self::PaymentMethod,
        Self::Category,
        Self::Frequency,
        Self::NextDue,
    ];
}

pub struct RecurringForm {
    pub mode: RecurringFormMode,
    pub description: InputField,
    pub amount: InputField,
    pub transaction_type: TransactionType,
    pub account_idx: usize,
    pub payment_method_idx: usize,
    pub category_idx: usize,
    pub frequency_idx: usize,
    pub next_due: InputField,
    pub active_field: usize,
    pub error: Option<String>,
}

pub struct ValidatedRecurring {
    pub description: String,
    pub amount: Decimal,
    pub transaction_type: TransactionType,
    pub account_id: i32,
    pub payment_method: PaymentMethod,
    pub category_id: i32,
    pub frequency: Frequency,
    pub next_due: NaiveDate,
}

pub const FREQUENCIES: [Frequency; 4] = [
    Frequency::Daily,
    Frequency::Weekly,
    Frequency::Monthly,
    Frequency::Yearly,
];

impl RecurringForm {
    pub fn validate(
        &self,
        accounts: &[Account],
        categories: &[Category],
    ) -> Result<ValidatedRecurring, String> {
        let description = self.description.value.trim().to_string();
        if description.is_empty() {
            return Err("Description is required".into());
        }

        let amount = parse_positive_amount(&self.amount.value)?;

        let account = accounts
            .get(self.account_idx)
            .ok_or("No account selected")?;
        let account_id = account.id;

        let methods = account.allowed_payment_methods();
        let payment_method = methods
            .get(self.payment_method_idx)
            .copied()
            .ok_or("No payment method selected")?;

        let transaction_type = self.transaction_type;
        let filtered_categories: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parsed_type() == transaction_type.category_type())
            .collect();
        let category_id = filtered_categories
            .get(self.category_idx)
            .map(|c| c.id)
            .ok_or("No category selected")?;

        let frequency = FREQUENCIES
            .get(self.frequency_idx)
            .copied()
            .ok_or("No frequency selected")?;

        let next_due = NaiveDate::parse_from_str(self.next_due.value.trim(), "%d-%m-%Y")
            .map_err(|_| "Invalid date (use DD-MM-YYYY)".to_string())?;

        Ok(ValidatedRecurring {
            description,
            amount,
            transaction_type,
            account_id,
            payment_method,
            category_id,
            frequency,
            next_due,
        })
    }

    pub fn new_create() -> Self {
        let today = Local::now().date_naive();
        Self {
            mode: RecurringFormMode::Create,
            description: InputField::new("Description"),
            amount: InputField::new("Amount"),
            transaction_type: TransactionType::Expense,
            account_idx: 0,
            payment_method_idx: 0,
            category_idx: 0,
            frequency_idx: 2, // default to Monthly
            next_due: InputField::new("Next Due").with_value(today.format("%d-%m-%Y").to_string()),
            active_field: 0,
            error: None,
        }
    }

    pub fn new_edit(
        r: &RecurringTransaction,
        accounts: &[Account],
        categories: &[Category],
    ) -> Self {
        let txn_type = r.parsed_type();

        let account_idx = accounts
            .iter()
            .position(|a| a.id == r.account_id)
            .unwrap_or(0);

        let payment_method_idx = accounts
            .get(account_idx)
            .map(|a| {
                a.allowed_payment_methods()
                    .iter()
                    .position(|m| *m == r.parsed_payment_method())
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let filtered_categories: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parsed_type() == txn_type.category_type())
            .collect();
        let category_idx = filtered_categories
            .iter()
            .position(|c| c.id == r.category_id)
            .unwrap_or(0);

        let frequency_idx = FREQUENCIES
            .iter()
            .position(|f| f.as_str() == r.frequency)
            .unwrap_or(2);

        Self {
            mode: RecurringFormMode::Edit(r.id),
            description: InputField::new("Description").with_value(&r.description),
            amount: InputField::new("Amount").with_value(r.amount.to_string()),
            transaction_type: txn_type,
            account_idx,
            payment_method_idx,
            category_idx,
            frequency_idx,
            next_due: InputField::new("Next Due")
                .with_value(r.next_due.format("%d-%m-%Y").to_string()),
            active_field: 0,
            error: None,
        }
    }

    pub fn active_field_id(&self) -> RecurringField {
        RecurringField::ALL[self.active_field.min(RecurringField::ALL.len() - 1)]
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.recurring_form.is_some() {
        render_form(frame, area, app);
    } else {
        render_list(frame, area, app);
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let [table_area, detail_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);

    let today = Local::now().date_naive();

    let header = Row::new([
        "Description",
        "Amount",
        "Frequency",
        "Next Due",
        "Account",
        "Category",
    ])
    .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .recurring_list
        .iter()
        .map(|r| {
            let account_name = app.account_name(r.account_id);
            let category_name = app.category_name(r.category_id);

            let due_style = if r.next_due <= today {
                Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new([
                r.description.clone(),
                format_brl(r.amount),
                r.parsed_frequency().label().to_string(),
                r.next_due.format("%d-%m-%Y").to_string(),
                account_name.to_string(),
                category_name.to_string(),
            ])
            .style(due_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(15),
            Constraint::Length(14),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Length(14),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Recurring Transactions"),
    )
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.recurring_table_state);

    let detail_content = match app
        .recurring_table_state
        .selected()
        .and_then(|i| app.recurring_list.get(i))
    {
        Some(r) => {
            let account_name = app.account_name(r.account_id);
            let category_name = app.category_name(r.category_id);

            let pending = if r.next_due <= today {
                " (PENDING)"
            } else {
                ""
            };

            vec![
                Line::from(format!(
                    " {} | {} | {}",
                    r.description, account_name, category_name,
                )),
                Line::from(format!(
                    " {} | {} | {} | Next: {}{}",
                    r.parsed_type().label(),
                    r.parsed_payment_method().label(),
                    r.parsed_frequency().label(),
                    r.next_due.format("%d-%m-%Y"),
                    pending,
                )),
                Line::from(format!(
                    " Amount: {} | Created: {}",
                    format_brl(r.amount),
                    r.created_at.format("%d-%m-%Y %H:%M"),
                )),
                Line::from(Span::styled(
                    " [c] Confirm pending  [n] New  [e] Edit  [d] Deactivate  [x] Export",
                    Style::new().fg(Color::DarkGray),
                )),
            ]
        }
        None => vec![Line::from(" No recurring transaction selected.")],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("Recurring Details");
    let detail = Paragraph::new(detail_content).block(detail_block);
    frame.render_widget(detail, detail_area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.recurring_form.as_ref().unwrap();

    let title = match form.mode {
        RecurringFormMode::Create => "New Recurring Transaction",
        RecurringFormMode::Edit(_) => "Edit Recurring Transaction",
    };

    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in RecurringField::ALL.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            RecurringField::Description => form.description.render_line(active),
            RecurringField::Amount => form.amount.render_line(active),
            RecurringField::TransactionType => render_toggle(
                "Type",
                &["Expense", "Income"],
                if form.transaction_type == TransactionType::Expense {
                    0
                } else {
                    1
                },
                active,
            ),
            RecurringField::Account => {
                let names: Vec<&str> = app.accounts.iter().map(|a| a.name.as_str()).collect();
                render_selector("Account", &names, form.account_idx, active, "no accounts")
            }
            RecurringField::PaymentMethod => {
                let methods: Vec<&str> = app
                    .accounts
                    .get(form.account_idx)
                    .map(|a| {
                        a.allowed_payment_methods()
                            .iter()
                            .map(|m| m.label())
                            .collect()
                    })
                    .unwrap_or_default();
                render_selector("Payment", &methods, form.payment_method_idx, active, "none")
            }
            RecurringField::Category => {
                let filtered: Vec<&str> = app
                    .categories
                    .iter()
                    .filter(|c| c.parsed_type() == form.transaction_type.category_type())
                    .map(|c| c.name.as_str())
                    .collect();
                render_selector("Category", &filtered, form.category_idx, active, "none")
            }
            RecurringField::Frequency => {
                let labels: Vec<&str> = FREQUENCIES.iter().map(|f| f.label()).collect();
                render_toggle("Frequency", &labels, form.frequency_idx, active)
            }
            RecurringField::NextDue => form.next_due.render_line(active),
        };
        lines.push(line);
    }

    push_form_error(&mut lines, &form.error);

    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

// ── Key handling & form submission (impl App) ─────────────────────

impl App {
    pub(crate) async fn handle_recurring_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_selection(
                    &mut self.recurring_table_state,
                    self.recurring_list.len(),
                    -1,
                );
            }
            KeyCode::Down | KeyCode::Char('j') => {
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
                    self.confirm_popup =
                        Some(ConfirmPopup::new(format!("Deactivate \"{}\"?", r_desc)));
                }
            }
            KeyCode::Char('c') => {
                self.confirm_recurring().await?;
            }
            KeyCode::Char('x') => {
                let acct_names = &self.account_names;
                let cats = &self.categories;
                match crate::export::export_recurring(
                    &self.recurring_list,
                    |id| acct_names.get(&id).cloned().unwrap_or_else(|| "?".into()),
                    |id| {
                        cats.iter()
                            .find(|c| c.id == id)
                            .map(|c| c.name.clone())
                            .unwrap_or_else(|| "?".into())
                    },
                ) {
                    Ok(path) => {
                        self.status_message = Some(StatusMessage::info(format!(
                            "Exported {} recurring transactions to {}",
                            self.recurring_list.len(),
                            path.display()
                        )));
                    }
                    Err(e) => {
                        self.status_message =
                            Some(StatusMessage::error(format!("Export failed: {e}")));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_recurring_form_key(&mut self, code: KeyCode) {
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

    pub(crate) async fn submit_recurring_form(&mut self) -> anyhow::Result<()> {
        use crate::db::recurring;

        let form = self.recurring_form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts, &self.categories) {
            Ok(v) => v,
            Err(e) => {
                self.recurring_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        let params = recurring::RecurringParams {
            amount: validated.amount,
            description: validated.description,
            category_id: validated.category_id,
            account_id: validated.account_id,
            transaction_type: validated.transaction_type,
            payment_method: validated.payment_method,
            frequency: validated.frequency,
            next_due: validated.next_due,
        };

        match form.mode {
            RecurringFormMode::Create => {
                recurring::create_recurring(&self.pool, &params).await?;
                info!(desc = %params.description, "recurring transaction created");
            }
            RecurringFormMode::Edit(id) => {
                recurring::update_recurring(&self.pool, id, &params).await?;
                info!(id, desc = %params.description, "recurring transaction updated");
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
        info!(
            id = r.id,
            desc = %r.description,
            next_due = %new_next_due,
            "recurring transaction confirmed"
        );

        self.load_data().await?;
        self.status_message = Some(StatusMessage::info(format!(
            "Confirmed \"{}\" — next due: {}",
            r.description,
            new_next_due.format("%d-%m-%Y")
        )));
        Ok(())
    }

    pub(crate) async fn execute_deactivate_recurring(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::recurring::deactivate_recurring(&self.pool, id).await?;
        info!(id, "recurring transaction deactivated");
        self.load_data().await?;
        clamp_selection(&mut self.recurring_table_state, self.recurring_list.len());
        Ok(())
    }
}
