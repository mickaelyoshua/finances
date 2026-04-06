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
        i18n::{Locale, t},
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
        locale: Locale,
    ) -> Result<ValidatedRecurring, String> {
        let description = self.description.value.trim().to_string();
        if description.is_empty() {
            return Err(t(locale, "err.description_required").into());
        }

        let amount = parse_positive_amount(&self.amount.value, locale)?;

        let account = accounts
            .get(self.account_idx)
            .ok_or(t(locale, "err.no_account"))?;
        let account_id = account.id;

        let methods = account.allowed_payment_methods();
        let payment_method = methods
            .get(self.payment_method_idx)
            .copied()
            .ok_or(t(locale, "err.no_payment_method"))?;

        let transaction_type = self.transaction_type;
        let filtered_categories: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parsed_type() == transaction_type.category_type())
            .collect();
        let category_id = filtered_categories
            .get(self.category_idx)
            .map(|c| c.id)
            .ok_or(t(locale, "err.no_category"))?;

        let frequency = FREQUENCIES
            .get(self.frequency_idx)
            .copied()
            .ok_or(t(locale, "err.no_frequency"))?;

        let next_due = NaiveDate::parse_from_str(self.next_due.value.trim(), "%d-%m-%Y")
            .map_err(|_| t(locale, "err.invalid_date").to_string())?;

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

    pub fn new_create(locale: Locale) -> Self {
        let today = Local::now().date_naive();
        Self {
            mode: RecurringFormMode::Create,
            description: InputField::new(t(locale, "form.description")),
            amount: InputField::new(t(locale, "form.amount")),
            transaction_type: TransactionType::Expense,
            account_idx: 0,
            payment_method_idx: 0,
            category_idx: 0,
            frequency_idx: 2, // default to Monthly
            next_due: InputField::new(t(locale, "form.next_due")).with_value(today.format("%d-%m-%Y").to_string()),
            active_field: 0,
            error: None,
        }
    }

    pub fn new_edit(
        r: &RecurringTransaction,
        accounts: &[Account],
        categories: &[Category],
        locale: Locale,
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
            description: InputField::new(t(locale, "form.description")).with_value(&r.description),
            amount: InputField::new(t(locale, "form.amount")).with_value(r.amount.to_string()),
            transaction_type: txn_type,
            account_idx,
            payment_method_idx,
            category_idx,
            frequency_idx,
            next_due: InputField::new(t(locale, "form.next_due"))
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
    if app.recur.form.is_some() {
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
        t(app.locale, "header.description"),
        t(app.locale, "header.amount"),
        t(app.locale, "header.frequency"),
        t(app.locale, "header.next_due"),
        t(app.locale, "header.account"),
        t(app.locale, "header.category"),
    ])
    .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .recur.list
        .iter()
        .map(|r| {
            let account_name = app.account_name(r.account_id);
            let category_name = app.category_name_localized(r.category_id);

            let due_style = if r.next_due <= today {
                Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new([
                r.description.clone(),
                format_brl(r.amount),
                app.locale.enum_label(r.parsed_frequency().label()).to_string(),
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
            .title(t(app.locale, "title.recurring")),
    )
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.recur.table_state);

    let detail_content = match app
        .recur.table_state
        .selected()
        .and_then(|i| app.recur.list.get(i))
    {
        Some(r) => {
            let account_name = app.account_name(r.account_id);
            let category_name = app.category_name_localized(r.category_id);

            let pending = if r.next_due <= today {
                format!(" {}", t(app.locale, "misc.pending"))
            } else {
                String::new()
            };

            vec![
                Line::from(format!(
                    " {} | {} | {}",
                    r.description, account_name, category_name,
                )),
                Line::from(format!(
                    " {} | {} | {} | {}: {}{}",
                    app.locale.enum_label(r.parsed_type().label()),
                    app.locale.enum_label(r.parsed_payment_method().label()),
                    app.locale.enum_label(r.parsed_frequency().label()),
                    t(app.locale, "detail.next"),
                    r.next_due.format("%d-%m-%Y"),
                    pending,
                )),
                Line::from(format!(
                    " {}: {} | {}: {}",
                    t(app.locale, "detail.amount"),
                    format_brl(r.amount),
                    t(app.locale, "detail.created"),
                    r.created_at.format("%d-%m-%Y %H:%M"),
                )),
                Line::from(Span::styled(
                    format!(" {}", t(app.locale, "hint.recurring")),
                    Style::new().fg(Color::DarkGray),
                )),
            ]
        }
        None => vec![Line::from(format!(" {}", t(app.locale, "misc.no_sel.recurring")))],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title(t(app.locale, "title.recurring_details"));
    let detail = Paragraph::new(detail_content).block(detail_block);
    frame.render_widget(detail, detail_area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.recur.form.as_ref().unwrap();

    let title = match form.mode {
        RecurringFormMode::Create => t(app.locale, "title.new_recurring"),
        RecurringFormMode::Edit(_) => t(app.locale, "title.edit_recurring"),
    };

    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in RecurringField::ALL.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            RecurringField::Description => form.description.render_line(active),
            RecurringField::Amount => form.amount.render_line(active),
            RecurringField::TransactionType => render_toggle(
                t(app.locale, "form.type"),
                &[app.locale.enum_label("Expense"), app.locale.enum_label("Income")],
                if form.transaction_type == TransactionType::Expense {
                    0
                } else {
                    1
                },
                active,
                area.width,
            ),
            RecurringField::Account => {
                let names: Vec<&str> = app.accounts.iter().map(|a| a.name.as_str()).collect();
                render_selector(t(app.locale, "form.account"), &names, form.account_idx, active, t(app.locale, "misc.no_accounts"), area.width)
            }
            RecurringField::PaymentMethod => {
                let methods: Vec<&str> = app
                    .accounts
                    .get(form.account_idx)
                    .map(|a| {
                        a.allowed_payment_methods()
                            .iter()
                            .map(|m| app.locale.enum_label(m.label()))
                            .collect()
                    })
                    .unwrap_or_default();
                render_selector(t(app.locale, "form.payment"), &methods, form.payment_method_idx, active, t(app.locale, "misc.none"), area.width)
            }
            RecurringField::Category => {
                let filtered: Vec<&str> = app
                    .categories
                    .iter()
                    .filter(|c| c.parsed_type() == form.transaction_type.category_type())
                    .map(|c| c.localized_name(app.locale))
                    .collect();
                render_selector(t(app.locale, "form.category"), &filtered, form.category_idx, active, t(app.locale, "misc.none"), area.width)
            }
            RecurringField::Frequency => {
                let labels: Vec<&str> = FREQUENCIES.iter().map(|f| app.locale.enum_label(f.label())).collect();
                render_toggle(t(app.locale, "form.frequency"), &labels, form.frequency_idx, active, area.width)
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
                    &mut self.recur.table_state,
                    self.recur.list.len(),
                    -1,
                );
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_selection(
                    &mut self.recur.table_state,
                    self.recur.list.len(),
                    1,
                );
            }
            KeyCode::Char('n') => {
                if self.accounts.is_empty() {
                    self.status_message = Some(StatusMessage::error(t(self.locale, "msg.create_account_first")));
                } else if self.categories.is_empty() {
                    self.status_message = Some(StatusMessage::error(t(self.locale, "msg.create_category_first")));
                } else {
                    self.recur.form = Some(RecurringForm::new_create(self.locale));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('e') => {
                if let Some(r) = self
                    .recur.table_state
                    .selected()
                    .and_then(|i| self.recur.list.get(i))
                {
                    let r = r.clone();
                    self.recur.form = Some(RecurringForm::new_edit(
                        &r,
                        &self.accounts,
                        &self.categories,
                        self.locale,
                    ));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(r) = self
                    .recur.table_state
                    .selected()
                    .and_then(|i| self.recur.list.get(i))
                {
                    let r_id = r.id;
                    let r_desc = r.description.clone();
                    self.confirm_action = Some(ConfirmAction::DeactivateRecurring(r_id));
                    self.confirm_popup = Some(ConfirmPopup::new(
                        crate::ui::i18n::tf_deactivate(self.locale, &r_desc, false)
                    ));
                }
            }
            KeyCode::Char('c') => {
                self.confirm_recurring().await?;
            }
            KeyCode::Char('x') => {
                let acct_names = &self.account_names;
                let cats = &self.categories;
                match crate::export::export_recurring(
                    &self.recur.list,
                    |id| acct_names.get(&id).cloned().unwrap_or_else(|| "?".into()),
                    |id| {
                        cats.iter()
                            .find(|c| c.id == id)
                            .map(|c| c.name.clone())
                            .unwrap_or_else(|| "?".into())
                    },
                ) {
                    Ok(path) => {
                        self.status_message = Some(StatusMessage::info(
                            crate::ui::i18n::tf_exported(self.locale, self.recur.list.len(), t(self.locale, "export.recurring"), &path)
                        ));
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage::error(
                            crate::ui::i18n::tf_export_failed(self.locale, &e)
                        ));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_recurring_form_key(&mut self, code: KeyCode) {
        let form = self.recur.form.as_mut().unwrap();
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

        let form = self.recur.form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts, &self.categories, self.locale) {
            Ok(v) => v,
            Err(e) => {
                self.recur.form.as_mut().unwrap().error = Some(e);
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

        self.recur.form = None;
        self.input_mode = InputMode::Normal;
        self.refresh_recurring().await?;
        Ok(())
    }

    async fn confirm_recurring(&mut self) -> anyhow::Result<()> {
        use crate::db::recurring;

        let today = Local::now().date_naive();

        let r = match self
            .recur.table_state
            .selected()
            .and_then(|i| self.recur.list.get(i))
        {
            Some(r) => r.clone(),
            None => return Ok(()),
        };

        if r.next_due > today {
            self.status_message = Some(StatusMessage::error(
                crate::ui::i18n::tf_not_due_yet(self.locale, &r.next_due.format("%d-%m-%Y").to_string())
            ));
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

        self.refresh_recurring().await?;
        self.load_transactions().await?;
        self.refresh_balances().await?;
        self.refresh_budgets().await?;
        self.status_message = Some(StatusMessage::info(
            crate::ui::i18n::tf_confirmed(self.locale, &r.description, &new_next_due.format("%d-%m-%Y").to_string())
        ));
        Ok(())
    }

    pub(crate) async fn execute_deactivate_recurring(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::recurring::deactivate_recurring(&self.pool, id).await?;
        info!(id, "recurring transaction deactivated");
        self.refresh_recurring().await?;
        clamp_selection(&mut self.recur.table_state, self.recur.list.len());
        Ok(())
    }
}
