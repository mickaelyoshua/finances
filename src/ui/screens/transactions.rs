use chrono::{Datelike, Local, NaiveDate};
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use rust_decimal::Decimal;
use tracing::{debug, info};

use crate::{
    db::{self, clamped_day, latest_closing_date, next_month, statement_due_date,
         transactions::TransactionFilterParams},
    models::{Account, Category, CategoryType, InstallmentPurchase, PaymentMethod, Transaction, TransactionType},
    ui::{
        App,
        app::{
            ConfirmAction, InputMode, PAGE_SIZE, StatusMessage, clamp_selection, cycle_index,
            is_toggle_key, move_table_selection,
        },
        components::{
            format::{format_brl, parse_positive_amount},
            input::InputField,
            popup::ConfirmPopup,
            toggle::{push_form_error, render_selector, render_toggle},
        },
        i18n::{Locale, t, tf_paginated},
    },
};

// ── Transaction form types ───────────────────────────────────────

pub enum TransactionFormMode {
    Create,
    Edit(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionField {
    Date,
    Description,
    Amount,
    IsInstallment,
    InstallmentCount,
    TransactionType,
    Account,
    PaymentMethod,
    Category,
}

impl TransactionField {
    pub const ALL: [Self; 9] = [
        Self::Date,
        Self::Description,
        Self::Amount,
        Self::IsInstallment,
        Self::InstallmentCount,
        Self::TransactionType,
        Self::Account,
        Self::PaymentMethod,
        Self::Category,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterField {
    DateFrom,
    DateTo,
    Description,
    Account,
    Category,
    TransactionType,
    PaymentMethod,
}

impl FilterField {
    pub const ALL: [Self; 7] = [
        Self::DateFrom,
        Self::DateTo,
        Self::Description,
        Self::Account,
        Self::Category,
        Self::TransactionType,
        Self::PaymentMethod,
    ];
}

/// State for the transaction filter bar (toggled with 'f').
/// Selector fields use `Option<usize>` where `None` = "All" (no filter).
pub struct TransactionFilter {
    pub date_from: InputField,
    pub date_to: InputField,
    pub description: InputField,
    pub account_idx: Option<usize>,
    pub category_idx: Option<usize>,
    pub transaction_type_idx: Option<usize>,
    pub payment_method_idx: Option<usize>,
    pub active_field: usize,
    pub visible: bool,
}

impl TransactionFilter {
    pub fn new(locale: Locale) -> Self {
        Self {
            date_from: InputField::new(t(locale, "form.from")),
            date_to: InputField::new(t(locale, "form.to")),
            description: InputField::new(t(locale, "form.desc")),
            account_idx: None,
            category_idx: None,
            transaction_type_idx: None,
            payment_method_idx: None,
            active_field: 0,
            visible: false,
        }
    }

    pub fn active_field_id(&self) -> FilterField {
        FilterField::ALL[self.active_field.min(FilterField::ALL.len() - 1)]
    }

    pub fn to_params(
        &self,
        accounts: &[Account],
        categories: &[Category],
    ) -> TransactionFilterParams {
        let date_from = NaiveDate::parse_from_str(self.date_from.value.trim(), "%d-%m-%Y").ok();
        let date_to = NaiveDate::parse_from_str(self.date_to.value.trim(), "%d-%m-%Y").ok();

        let description = {
            let d = self.description.value.trim().to_string();
            if d.is_empty() { None } else { Some(d) }
        };

        let account_id = self.account_idx.and_then(|i| accounts.get(i)).map(|a| a.id);

        let category_id = self
            .category_idx
            .and_then(|i| categories.get(i))
            .map(|c| c.id);

        let transaction_type = self.transaction_type_idx.map(|i| {
            if i == 0 {
                TransactionType::Expense
            } else {
                TransactionType::Income
            }
        });

        let payment_method = self.payment_method_idx.and_then(|i| {
            [
                PaymentMethod::Pix,
                PaymentMethod::Credit,
                PaymentMethod::Debit,
                PaymentMethod::Cash,
                PaymentMethod::Boleto,
                PaymentMethod::Transfer,
            ]
            .get(i)
            .copied()
        });

        TransactionFilterParams {
            date_from,
            date_to,
            account_id,
            category_id,
            transaction_type,
            payment_method,
            description,
        }
    }
}

impl Default for TransactionFilter {
    fn default() -> Self {
        Self::new(Locale::default())
    }
}

/// Cycle through `None → Some(0) → … → Some(len-1) → None`.
/// Used by filter selectors so "All" (None) is reachable by cycling past the last option.
/// Direction: `Left` cycles backward, `Right`/`Space` cycle forward.
pub fn cycle_option(current: Option<usize>, len: usize, code: KeyCode) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match code {
        KeyCode::Left => match current {
            None => Some(len - 1),
            Some(0) => None,
            Some(i) => Some(i - 1),
        },
        _ => match current {
            None => Some(0),
            Some(i) if i + 1 < len => Some(i + 1),
            Some(_) => None,
        },
    }
}

// ── Installment confirmation (shared by create and edit flows) ───

pub struct InstallmentConfirmation {
    pub validated: ValidatedInstallment,
    pub per_installment: Decimal,
    pub parcela_dues: Vec<(i16, NaiveDate)>,
    pub confirmed: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedInstallment {
    pub description: String,
    pub total_amount: Decimal,
    pub installment_count: i16,
    pub first_date: NaiveDate,
    pub account_id: i32,
    pub category_id: i32,
}

// ── Transaction form ─────────────────────────────────────────────

pub struct TransactionForm {
    pub mode: TransactionFormMode,
    pub date: InputField,
    pub description: InputField,
    pub amount: InputField,
    pub transaction_type: TransactionType,
    pub account_idx: usize,
    pub payment_method_idx: usize,
    pub category_idx: usize,
    pub active_field: usize,
    pub error: Option<String>,
    pub is_installment: bool,
    pub installment_count: InputField,
    pub confirmation: Option<InstallmentConfirmation>,
}

#[derive(Debug)]
pub struct ValidatedTransaction {
    pub date: NaiveDate,
    pub description: String,
    pub amount: Decimal,
    pub account_id: i32,
    pub payment_method: PaymentMethod,
    pub category_id: i32,
    pub transaction_type: TransactionType,
}

impl TransactionForm {
    /// Returns the list of fields visible in the current form mode.
    pub fn visible_fields(&self) -> Vec<TransactionField> {
        TransactionField::ALL
            .iter()
            .copied()
            .filter(|f| match f {
                TransactionField::IsInstallment => matches!(self.mode, TransactionFormMode::Create),
                TransactionField::InstallmentCount => {
                    matches!(self.mode, TransactionFormMode::Create) && self.is_installment
                }
                _ => true,
            })
            .collect()
    }

    pub fn validate(
        &self,
        accounts: &[Account],
        categories: &[Category],
        locale: Locale,
    ) -> Result<ValidatedTransaction, String> {
        let date = NaiveDate::parse_from_str(self.date.value.trim(), "%d-%m-%Y")
            .map_err(|_| t(locale, "err.invalid_date").to_string())?;

        let description = self.description.value.trim().to_string();
        if description.is_empty() {
            return Err(t(locale, "err.description_required").into());
        }

        let amount = parse_positive_amount(&self.amount.value, locale)?;

        let effective_accounts = self.effective_accounts(accounts);
        let account = effective_accounts
            .get(self.account_idx)
            .ok_or_else(|| t(locale, "err.no_account"))?;
        let account_id = account.id;

        let transaction_type = if self.is_installment {
            TransactionType::Expense
        } else {
            self.transaction_type
        };

        let payment_method = if self.is_installment {
            PaymentMethod::Credit
        } else {
            let methods = account.allowed_payment_methods();
            methods
                .get(self.payment_method_idx)
                .copied()
                .ok_or_else(|| t(locale, "err.no_payment_method"))?
        };

        let filtered_categories: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parsed_type() == transaction_type.category_type())
            .collect();
        let category_id = filtered_categories
            .get(self.category_idx)
            .map(|c| c.id)
            .ok_or_else(|| t(locale, "err.no_category"))?;

        Ok(ValidatedTransaction {
            date,
            description,
            amount,
            account_id,
            payment_method,
            category_id,
            transaction_type,
        })
    }

    /// Validate the installment-specific fields (count >= 2).
    pub fn validate_installment_count(&self, locale: Locale) -> Result<i16, String> {
        self.installment_count
            .value
            .trim()
            .parse::<i16>()
            .map_err(|_| t(locale, "err.invalid_inst_count").to_string())
            .and_then(|v| {
                if v >= 2 {
                    Ok(v)
                } else {
                    Err(t(locale, "err.at_least_two_inst").into())
                }
            })
    }

    /// Returns accounts filtered by installment mode (CC-only when installment).
    pub fn effective_accounts<'a>(&self, accounts: &'a [Account]) -> Vec<&'a Account> {
        if self.is_installment {
            accounts.iter().filter(|a| a.has_credit_card).collect()
        } else {
            accounts.iter().collect()
        }
    }

    pub fn new_create(locale: Locale) -> Self {
        let today = Local::now().date_naive();

        Self {
            mode: TransactionFormMode::Create,
            date: InputField::new(t(locale, "form.date")).with_value(today.format("%d-%m-%Y").to_string()),
            description: InputField::new(t(locale, "form.description")),
            amount: InputField::new(t(locale, "form.amount")),
            transaction_type: TransactionType::Expense,
            account_idx: 0,
            payment_method_idx: 0,
            category_idx: 0,
            active_field: 0,
            error: None,
            is_installment: false,
            installment_count: InputField::new(t(locale, "form.installments")),
            confirmation: None,
        }
    }

    pub fn new_edit(txn: &Transaction, accounts: &[Account], categories: &[Category], locale: Locale) -> Self {
        let txn_type = txn.parsed_type();

        let account_idx = accounts
            .iter()
            .position(|a| a.id == txn.account_id)
            .unwrap_or(0);

        let payment_method_idx = accounts
            .get(account_idx)
            .map(|a| {
                a.allowed_payment_methods()
                    .iter()
                    .position(|m| *m == txn.parsed_payment_method())
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        let filtered_categories: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parsed_type() == txn_type.category_type())
            .collect();
        let category_idx = filtered_categories
            .iter()
            .position(|c| c.id == txn.category_id)
            .unwrap_or(0);

        Self {
            mode: TransactionFormMode::Edit(txn.id),
            date: InputField::new(t(locale, "form.date")).with_value(txn.date.format("%d-%m-%Y").to_string()),
            description: InputField::new(t(locale, "form.description")).with_value(&txn.description),
            amount: InputField::new(t(locale, "form.amount")).with_value(txn.amount.to_string()),
            transaction_type: txn_type,
            account_idx,
            payment_method_idx,
            category_idx,
            active_field: 0,
            error: None,
            is_installment: false,
            installment_count: InputField::new(t(locale, "form.installments")),
            confirmation: None,
        }
    }

    pub fn active_field_id(&self) -> TransactionField {
        let fields = self.visible_fields();
        fields[self.active_field.min(fields.len() - 1)]
    }
}

// ── Installment edit form (for editing existing installment groups) ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallmentFormMode {
    Create,
    Edit(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallmentField {
    Description,
    TotalAmount,
    InstallmentCount,
    FirstDate,
    Account,
    Category,
}

impl InstallmentField {
    pub const ALL: [Self; 6] = [
        Self::Description,
        Self::TotalAmount,
        Self::InstallmentCount,
        Self::FirstDate,
        Self::Account,
        Self::Category,
    ];
}

pub struct InstallmentForm {
    pub mode: InstallmentFormMode,
    pub description: InputField,
    pub total_amount: InputField,
    pub installment_count: InputField,
    pub first_date: InputField,
    pub account_idx: usize,
    pub category_idx: usize,
    pub active_field: usize,
    pub error: Option<String>,
    pub confirmation: Option<InstallmentConfirmation>,
}

impl InstallmentForm {
    pub fn validate(
        &self,
        accounts: &[Account],
        categories: &[Category],
        locale: Locale,
    ) -> Result<ValidatedInstallment, String> {
        let description = self.description.value.trim().to_string();
        if description.is_empty() {
            return Err(t(locale, "err.description_required").into());
        }

        let total_amount = parse_positive_amount(&self.total_amount.value, locale)?;

        let installment_count = self
            .installment_count
            .value
            .trim()
            .parse::<i16>()
            .map_err(|_| t(locale, "err.invalid_inst_count").to_string())
            .and_then(|v| {
                if v >= 2 {
                    Ok(v)
                } else {
                    Err(t(locale, "err.at_least_two_inst").into())
                }
            })?;

        let first_date = NaiveDate::parse_from_str(self.first_date.value.trim(), "%d-%m-%Y")
            .map_err(|_| t(locale, "err.invalid_date").to_string())?;

        let credit_accounts: Vec<&Account> =
            accounts.iter().filter(|a| a.has_credit_card).collect();
        let account_id = credit_accounts
            .get(self.account_idx)
            .map(|a| a.id)
            .ok_or(t(locale, "err.no_account"))?;

        let expense_categories: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parsed_type() == CategoryType::Expense)
            .collect();
        let category_id = expense_categories
            .get(self.category_idx)
            .map(|c| c.id)
            .ok_or(t(locale, "err.no_category"))?;

        Ok(ValidatedInstallment {
            description,
            total_amount,
            installment_count,
            first_date,
            account_id,
            category_id,
        })
    }

    pub fn new_edit(ip: &InstallmentPurchase, accounts: &[Account], categories: &[Category], locale: Locale) -> Self {
        let credit_accounts: Vec<&Account> =
            accounts.iter().filter(|a| a.has_credit_card).collect();
        let account_idx = credit_accounts
            .iter()
            .position(|a| a.id == ip.account_id)
            .unwrap_or(0);

        let expense_categories: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parsed_type() == CategoryType::Expense)
            .collect();
        let category_idx = expense_categories
            .iter()
            .position(|c| c.id == ip.category_id)
            .unwrap_or(0);

        Self {
            mode: InstallmentFormMode::Edit(ip.id),
            description: InputField::new(t(locale, "form.description")).with_value(&ip.description),
            total_amount: InputField::new(t(locale, "form.total_amount")).with_value(ip.total_amount.to_string()),
            installment_count: InputField::new(t(locale, "form.installments"))
                .with_value(ip.installment_count.to_string()),
            first_date: InputField::new(t(locale, "form.purchase_date"))
                .with_value(ip.first_installment_date.format("%d-%m-%Y").to_string()),
            account_idx,
            category_idx,
            active_field: 0,
            error: None,
            confirmation: None,
        }
    }

    pub fn active_field_id(&self) -> InstallmentField {
        InstallmentField::ALL[self.active_field.min(InstallmentField::ALL.len() - 1)]
    }
}

// ── Rendering ────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if let Some(form) = &app.txn.inst_form {
        if form.confirmation.is_some() {
            render_inst_confirmation(frame, area, app);
        } else {
            render_inst_form(frame, area, app);
        }
    } else if let Some(form) = &app.txn.form {
        if form.confirmation.is_some() {
            render_txn_confirmation(frame, area, app);
        } else {
            render_form(frame, area, app);
        }
    } else {
        render_list(frame, area, app);
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let filter_height = if app.txn.filter.visible { 4 } else { 0 };

    let [filter_area, table_area, detail_area] = Layout::vertical([
        Constraint::Length(filter_height),
        Constraint::Min(5),
        Constraint::Length(7),
    ])
    .areas(area);

    if app.txn.filter.visible {
        render_filter_bar(frame, filter_area, app);
    }

    let header = Row::new([
        t(app.locale, "header.date"),
        t(app.locale, "header.description"),
        t(app.locale, "header.amount"),
        t(app.locale, "header.account"),
        t(app.locale, "header.category"),
    ])
    .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .txn.items
        .iter()
        .map(|txn| {
            let account_name = app.account_name(txn.account_id);
            let category_name = app.category_name_localized(txn.category_id);

            let amount_str = if txn.parsed_type() == TransactionType::Expense {
                format!("-{}", format_brl(txn.amount))
            } else {
                format!("+{}", format_brl(txn.amount))
            };

            Row::new([
                txn.date.format("%d-%m-%Y").to_string(),
                txn.description.clone(),
                amount_str,
                account_name.to_string(),
                category_name.to_string(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Min(15),
            Constraint::Length(16),
            Constraint::Length(14),
            Constraint::Length(14),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(if app.txn.count > 0 {
                let start = app.txn.offset + 1;
                let end = app.txn.offset + app.txn.items.len() as u64;
                tf_paginated(app.locale, t(app.locale, "title.transactions"), start, end, app.txn.count)
            } else {
                format!("{} (0)", t(app.locale, "title.transactions"))
            }),
    )
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.txn.table_state);

    let detail_content = match app
        .txn.table_state
        .selected()
        .and_then(|i| app.txn.items.get(i))
    {
        Some(txn) => {
            let account_name = app.account_name(txn.account_id);

            let installment_info = match (txn.installment_purchase_id, txn.installment_number) {
                (Some(_), Some(n)) => format!(" | {} #{n}", t(app.locale, "detail.installment")),
                _ => String::new(),
            };

            vec![
                Line::from(format!(
                    " {} | {} | {}",
                    txn.date.format("%d-%m-%Y"),
                    app.locale.enum_label(txn.parsed_type().label()),
                    app.locale.enum_label(txn.parsed_payment_method().label()),
                )),
                Line::from(format!(
                    " {} | {} | {}{}",
                    txn.description,
                    format_brl(txn.amount),
                    account_name,
                    installment_info,
                )),
                Line::from(format!(
                    " {}: {}",
                    t(app.locale, "misc.created"),
                    txn.created_at.format("%d-%m-%Y %H:%M")
                )),
                Line::from(Span::styled(
                    format!(" {}", t(app.locale, "hint.txn")),
                    Style::new().fg(Color::DarkGray),
                )),
            ]
        }
        None => vec![Line::from(format!(" {}", t(app.locale, "misc.no_sel.transaction")))],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title(t(app.locale, "title.transaction_details"));
    let detail = Paragraph::new(detail_content).block(detail_block);
    frame.render_widget(detail, detail_area);
}

fn render_filter_bar(frame: &mut Frame, area: Rect, app: &mut App) {
    let filter = &app.txn.filter;
    let is_filtering = app.input_mode == InputMode::Filtering;
    let border_color = if is_filtering {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    // Row 1: DateFrom | DateTo | Description (text inputs)
    let mut row1: Vec<Span> = Vec::new();
    for (i, field) in [
        FilterField::DateFrom,
        FilterField::DateTo,
        FilterField::Description,
    ]
    .iter()
    .enumerate()
    {
        let idx = FilterField::ALL.iter().position(|f| f == field).unwrap();
        let active = is_filtering && filter.active_field == idx;
        if i > 0 {
            row1.push(Span::raw(" | "));
        }
        let input = match field {
            FilterField::DateFrom => &filter.date_from,
            FilterField::DateTo => &filter.date_to,
            FilterField::Description => &filter.description,
            _ => unreachable!(),
        };
        row1.extend(input.render_inline_spans(active));
    }

    // Row 2: Account | Category | Type | PaymentMethod (cycling selectors)
    let mut row2: Vec<Span> = Vec::new();
    let selector_fields = [
        FilterField::Account,
        FilterField::Category,
        FilterField::TransactionType,
        FilterField::PaymentMethod,
    ];
    for (i, field) in selector_fields.iter().enumerate() {
        let idx = FilterField::ALL.iter().position(|f| f == field).unwrap();
        let active = is_filtering && filter.active_field == idx;
        if i > 0 {
            row2.push(Span::raw(" | "));
        }
        let label_style = if active {
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::DarkGray)
        };

        let all = t(app.locale, "filter.all");
        let (label, value) = match field {
            FilterField::Account => (
                t(app.locale, "filter.acct"),
                filter
                    .account_idx
                    .and_then(|i| app.accounts.get(i))
                    .map(|a| a.name.as_str())
                    .unwrap_or(all),
            ),
            FilterField::Category => (
                t(app.locale, "filter.cat"),
                filter
                    .category_idx
                    .and_then(|i| app.categories.get(i))
                    .map(|c| c.localized_name(app.locale))
                    .unwrap_or(all),
            ),
            FilterField::TransactionType => (
                t(app.locale, "filter.type"),
                match filter.transaction_type_idx {
                    Some(0) => app.locale.enum_label("Expense"),
                    Some(_) => app.locale.enum_label("Income"),
                    None => all,
                },
            ),
            FilterField::PaymentMethod => {
                let pm_value = filter
                    .payment_method_idx
                    .and_then(|i| {
                        [
                            PaymentMethod::Pix,
                            PaymentMethod::Credit,
                            PaymentMethod::Debit,
                            PaymentMethod::Cash,
                            PaymentMethod::Boleto,
                            PaymentMethod::Transfer,
                        ]
                        .get(i)
                        .map(|m| app.locale.enum_label(m.label()))
                    })
                    .unwrap_or(all);
                (t(app.locale, "filter.pay"), pm_value)
            }
            _ => unreachable!(),
        };

        row2.push(Span::styled(format!("{label}: "), label_style));
        let value_style = if active {
            Style::new().fg(Color::Cyan)
        } else {
            Style::default()
        };
        row2.push(Span::styled(value, value_style));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(t(app.locale, "title.filter"))
        .border_style(Style::new().fg(border_color));

    let lines = vec![Line::from(row1), Line::from(row2)];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.txn.form.as_ref().unwrap();

    let title = match form.mode {
        TransactionFormMode::Create => t(app.locale, "title.new_transaction"),
        TransactionFormMode::Edit(_) => t(app.locale, "title.edit_transaction"),
    };

    let visible = form.visible_fields();
    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in visible.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            TransactionField::Date => form.date.render_line(active),
            TransactionField::Description => form.description.render_line(active),
            TransactionField::Amount => form.amount.render_line(active),
            TransactionField::IsInstallment => {
                let labels = [t(app.locale, "misc.no"), t(app.locale, "misc.yes")];
                render_toggle(
                    t(app.locale, "form.is_installment"),
                    &labels,
                    if form.is_installment { 1 } else { 0 },
                    active,
                    area.width,
                )
            }
            TransactionField::InstallmentCount => form.installment_count.render_line(active),
            TransactionField::TransactionType => {
                if form.is_installment {
                    // Locked to Expense
                    Line::from(vec![
                        Span::styled(
                            format!(" {}: ", t(app.locale, "form.type")),
                            Style::new().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("{} ({})", app.locale.enum_label("Expense"), t(app.locale, "misc.locked")),
                            Style::new().fg(Color::DarkGray),
                        ),
                    ])
                } else {
                    let labels: Vec<&str> = [TransactionType::Expense, TransactionType::Income]
                        .iter()
                        .map(|tt| app.locale.enum_label(tt.label()))
                        .collect();
                    render_toggle(
                        t(app.locale, "form.type"),
                        &labels,
                        if form.transaction_type == TransactionType::Expense {
                            0
                        } else {
                            1
                        },
                        active,
                        area.width,
                    )
                }
            }
            TransactionField::Account => {
                if form.is_installment {
                    let names: Vec<&str> = app.accounts.iter()
                        .filter(|a| a.has_credit_card)
                        .map(|a| a.name.as_str())
                        .collect();
                    render_selector(t(app.locale, "form.account"), &names, form.account_idx, active, t(app.locale, "misc.no_accounts_cc"), area.width)
                } else {
                    let names: Vec<&str> = app.accounts.iter().map(|a| a.name.as_str()).collect();
                    render_selector(t(app.locale, "form.account"), &names, form.account_idx, active, t(app.locale, "misc.no_accounts"), area.width)
                }
            }
            TransactionField::PaymentMethod => {
                if form.is_installment {
                    // Locked to Credit
                    Line::from(vec![
                        Span::styled(
                            format!(" {}: ", t(app.locale, "form.payment")),
                            Style::new().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("{} ({})", app.locale.enum_label("Credit Card"), t(app.locale, "misc.locked")),
                            Style::new().fg(Color::DarkGray),
                        ),
                    ])
                } else {
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
            }
            TransactionField::Category => {
                let type_filter = if form.is_installment {
                    CategoryType::Expense
                } else {
                    form.transaction_type.category_type()
                };
                let filtered: Vec<&str> = app
                    .categories
                    .iter()
                    .filter(|c| c.parsed_type() == type_filter)
                    .map(|c| c.localized_name(app.locale))
                    .collect();
                render_selector(t(app.locale, "form.category"), &filtered, form.category_idx, active, t(app.locale, "misc.none"), area.width)
            }
        };
        lines.push(line);
    }

    push_form_error(&mut lines, &form.error);

    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Render the installment confirmation screen (shared by create via TransactionForm).
fn render_txn_confirmation(frame: &mut Frame, area: Rect, app: &mut App) {
    let locale = app.locale;
    let conf = app.txn.form.as_ref().unwrap().confirmation.as_ref().unwrap();
    render_confirmation_content(frame, area, locale, conf, t(locale, "title.confirm_installment"));
}

/// Render the installment confirmation screen for the InstallmentForm (edit flow).
fn render_inst_confirmation(frame: &mut Frame, area: Rect, app: &mut App) {
    let locale = app.locale;
    let form = app.txn.inst_form.as_ref().unwrap();
    let title = match form.mode {
        InstallmentFormMode::Create => t(locale, "title.confirm_installment"),
        InstallmentFormMode::Edit(_) => t(locale, "title.confirm_installment_edit"),
    };
    let conf = form.confirmation.as_ref().unwrap();
    render_confirmation_content(frame, area, locale, conf, title);
}

/// Shared rendering logic for installment confirmation screens.
fn render_confirmation_content(frame: &mut Frame, area: Rect, locale: Locale, conf: &InstallmentConfirmation, title: &str) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::raw(" "),
        Span::styled(
            &conf.validated.description,
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " — {} x {}",
                conf.validated.installment_count,
                format_brl(conf.per_installment)
            ),
            Style::new().fg(Color::DarkGray),
        ),
    ]));
    lines.push(Line::from(""));

    for (parcela, due) in &conf.parcela_dues {
        lines.push(Line::from(format!(
            "  {} {:>2} → {} {}",
            t(locale, "misc.parcela"),
            parcela,
            t(locale, "misc.due"),
            due.format("%d/%m/%Y")
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(" {}", t(locale, "misc.is_this_correct")),
        Style::new().fg(Color::Yellow),
    )));
    lines.push(Line::from(""));

    let yes_style = if conf.confirmed {
        Style::new()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::DarkGray)
    };
    let no_style = if !conf.confirmed {
        Style::new()
            .fg(Color::Black)
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::DarkGray)
    };

    lines.push(Line::from(vec![
        Span::raw("   "),
        Span::styled(format!(" {} ", t(locale, "misc.yes")), yes_style),
        Span::raw("  "),
        Span::styled(format!(" {} ", t(locale, "misc.no")), no_style),
    ]));

    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Render the InstallmentForm (used for editing existing installment groups).
fn render_inst_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.txn.inst_form.as_ref().unwrap();

    let title = t(app.locale, "title.edit_installment");

    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in InstallmentField::ALL.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            InstallmentField::Description => form.description.render_line(active),
            InstallmentField::TotalAmount => form.total_amount.render_line(active),
            InstallmentField::InstallmentCount => form.installment_count.render_line(active),
            InstallmentField::FirstDate => form.first_date.render_line(active),
            InstallmentField::Account => {
                let names: Vec<&str> = app
                    .accounts
                    .iter()
                    .filter(|a| a.has_credit_card)
                    .map(|a| a.name.as_str())
                    .collect();
                // Account is always locked in edit mode
                let name = names.get(form.account_idx).unwrap_or(&"?");
                Line::from(vec![
                    Span::styled(
                        format!(" {}: ", t(app.locale, "form.account")),
                        Style::new().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{} ({})", name, t(app.locale, "misc.locked")),
                        Style::new().fg(Color::DarkGray),
                    ),
                ])
            }
            InstallmentField::Category => {
                let names: Vec<&str> = app
                    .categories
                    .iter()
                    .filter(|c| c.parsed_type() == CategoryType::Expense)
                    .map(|c| c.localized_name(app.locale))
                    .collect();
                render_selector(
                    t(app.locale, "form.category"),
                    &names,
                    form.category_idx,
                    active,
                    t(app.locale, "misc.no_expense_cats"),
                    area.width,
                )
            }
        };
        lines.push(line);
    }

    push_form_error(&mut lines, &form.error);

    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

// ── Key handling & form submission (impl App) ─────────────────────

impl App {
    pub(crate) async fn handle_transactions_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_selection(
                    &mut self.txn.table_state,
                    self.txn.items.len(),
                    -1,
                );
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_selection(
                    &mut self.txn.table_state,
                    self.txn.items.len(),
                    1,
                );
            }
            KeyCode::Char('n') => {
                if self.accounts.is_empty() {
                    self.status_message = Some(StatusMessage::error(t(self.locale, "msg.create_account_first")));
                } else if self.categories.is_empty() {
                    self.status_message = Some(StatusMessage::error(t(self.locale, "msg.create_category_first")));
                } else {
                    self.txn.form = Some(TransactionForm::new_create(self.locale));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('e') => {
                if let Some(txn) = self
                    .txn.table_state
                    .selected()
                    .and_then(|i| self.txn.items.get(i))
                {
                    if let Some(ip_id) = txn.installment_purchase_id {
                        // Edit the parent installment group
                        if let Some(ip) = self.installment_purchases.iter().find(|ip| ip.id == ip_id) {
                            let ip = ip.clone();
                            self.txn.inst_form = Some(InstallmentForm::new_edit(
                                &ip,
                                &self.accounts,
                                &self.categories,
                                self.locale,
                            ));
                            self.input_mode = InputMode::Editing;
                        }
                    } else {
                        let txn = txn.clone();
                        self.txn.form = Some(TransactionForm::new_edit(
                            &txn,
                            &self.accounts,
                            &self.categories,
                            self.locale,
                        ));
                        self.input_mode = InputMode::Editing;
                    }
                }
            }
            KeyCode::Char('d') => {
                if let Some(txn) = self
                    .txn.table_state
                    .selected()
                    .and_then(|i| self.txn.items.get(i))
                {
                    if let Some(ip_id) = txn.installment_purchase_id {
                        // Delete the entire installment group
                        if let Some(ip) = self.installment_purchases.iter().find(|ip| ip.id == ip_id) {
                            let ip_desc = ip.description.clone();
                            self.confirm_action = Some(ConfirmAction::DeleteInstallment(ip_id));
                            self.confirm_popup = Some(ConfirmPopup::new(
                                crate::ui::i18n::tf_delete_installment(self.locale, &ip_desc),
                            ));
                        }
                    } else {
                        let txn_id = txn.id;
                        let txn_desc = txn.description.clone();
                        self.confirm_action = Some(ConfirmAction::DeleteTransaction(txn_id));
                        self.confirm_popup = Some(ConfirmPopup::new(
                            crate::ui::i18n::tf_delete(self.locale, &txn_desc),
                        ));
                    }
                }
            }
            KeyCode::Char('f') => {
                self.txn.filter.visible = true;
                self.txn.filter.active_field = 0;
                self.input_mode = InputMode::Filtering;
            }
            KeyCode::Char('r') => {
                self.txn.filter = TransactionFilter::new(self.locale);
                self.txn.offset = 0;
                self.load_transactions().await?;
                self.txn.table_state.select(Some(0));
            }
            KeyCode::Char('x') => {
                let params = self
                    .txn.filter
                    .to_params(&self.accounts, &self.categories);
                match crate::db::transactions::list_all_filtered(&self.pool, &params).await {
                    Ok(all_txns) => {
                        let acct_names = &self.account_names;
                        let cats = &self.categories;
                        match crate::export::export_transactions(
                            &all_txns,
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
                                    crate::ui::i18n::tf_exported(self.locale, all_txns.len(), t(self.locale, "export.transactions"), &path)
                                ));
                            }
                            Err(e) => {
                                self.status_message = Some(StatusMessage::error(
                                    crate::ui::i18n::tf_export_failed(self.locale, &e)
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        self.status_message = Some(StatusMessage::error(
                            crate::ui::i18n::tf_export_failed(self.locale, &e)
                        ));
                    }
                }
            }
            KeyCode::PageUp => {
                if self.txn.offset > 0 {
                    self.txn.offset = self.txn.offset.saturating_sub(PAGE_SIZE);
                    self.load_transactions().await?;
                    self.txn.table_state.select(Some(0));
                }
            }
            KeyCode::PageDown => {
                let next = self.txn.offset + PAGE_SIZE;
                if next < self.txn.count {
                    self.txn.offset = next;
                    self.load_transactions().await?;
                    self.txn.table_state.select(Some(0));
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) async fn handle_filtering_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                self.txn.offset = 0;
                self.load_transactions().await?;
                self.txn.table_state.select(Some(0));
                self.input_mode = InputMode::Normal;
                debug!(count = self.txn.count, "filters applied");
            }
            KeyCode::Tab | KeyCode::Down => {
                if self.txn.filter.active_field < FilterField::ALL.len() - 1 {
                    self.txn.filter.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if self.txn.filter.active_field > 0 {
                    self.txn.filter.active_field -= 1;
                }
            }
            _ => match self.txn.filter.active_field_id() {
                FilterField::DateFrom => {
                    self.txn.filter.date_from.handle_key(key.code);
                }
                FilterField::DateTo => {
                    self.txn.filter.date_to.handle_key(key.code);
                }
                FilterField::Description => {
                    self.txn.filter.description.handle_key(key.code);
                }
                FilterField::Account => {
                    if is_toggle_key(key.code) {
                        let len = self.accounts.len();
                        self.txn.filter.account_idx =
                            cycle_option(self.txn.filter.account_idx, len, key.code);
                    }
                }
                FilterField::Category => {
                    if is_toggle_key(key.code) {
                        let len = self.categories.len();
                        self.txn.filter.category_idx =
                            cycle_option(self.txn.filter.category_idx, len, key.code);
                    }
                }
                FilterField::TransactionType => {
                    if is_toggle_key(key.code) {
                        self.txn.filter.transaction_type_idx =
                            cycle_option(self.txn.filter.transaction_type_idx, 2, key.code);
                    }
                }
                FilterField::PaymentMethod => {
                    if is_toggle_key(key.code) {
                        self.txn.filter.payment_method_idx =
                            cycle_option(self.txn.filter.payment_method_idx, 6, key.code);
                    }
                }
            },
        }
        Ok(())
    }

    pub(crate) fn handle_transaction_form_key(&mut self, code: KeyCode) {
        let form = self.txn.form.as_mut().unwrap();

        // If confirming (installment), only handle Left/Right toggle
        if let Some(conf) = &mut form.confirmation {
            if matches!(code, KeyCode::Left | KeyCode::Right) {
                conf.confirmed = !conf.confirmed;
            }
            return;
        }

        let visible = form.visible_fields();
        match code {
            KeyCode::Tab | KeyCode::Down => {
                if form.active_field < visible.len() - 1 {
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
                TransactionField::IsInstallment => {
                    if is_toggle_key(code) {
                        form.is_installment = !form.is_installment;
                        // Reset dependent fields
                        form.account_idx = 0;
                        form.payment_method_idx = 0;
                        form.category_idx = 0;
                        if form.is_installment {
                            form.transaction_type = TransactionType::Expense;
                        }
                    }
                }
                TransactionField::InstallmentCount => form.installment_count.handle_key(code),
                TransactionField::TransactionType => {
                    if !form.is_installment && is_toggle_key(code) {
                        form.transaction_type = match form.transaction_type {
                            TransactionType::Expense => TransactionType::Income,
                            TransactionType::Income => TransactionType::Expense,
                        };
                        form.category_idx = 0;
                    }
                }
                TransactionField::Account => {
                    if is_toggle_key(code) {
                        let len = form.effective_accounts(&self.accounts).len();
                        cycle_index(&mut form.account_idx, len, code);
                        form.payment_method_idx = 0;
                    }
                }
                TransactionField::PaymentMethod => {
                    if !form.is_installment && is_toggle_key(code) {
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
                        let type_filter = if form.is_installment {
                            CategoryType::Expense
                        } else {
                            form.transaction_type.category_type()
                        };
                        let len = self
                            .categories
                            .iter()
                            .filter(|c| c.parsed_type() == type_filter)
                            .count();
                        cycle_index(&mut form.category_idx, len, code);
                    }
                }
            },
        }
    }

    pub(crate) fn handle_installment_form_key(&mut self, code: KeyCode) {
        let form = self.txn.inst_form.as_mut().unwrap();

        // If confirming, only handle Left/Right toggle
        if let Some(conf) = &mut form.confirmation {
            if matches!(code, KeyCode::Left | KeyCode::Right) {
                conf.confirmed = !conf.confirmed;
            }
            return;
        }

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
                    // Account is always locked in edit mode (only mode used)
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

    pub(crate) async fn submit_transaction_form(&mut self) -> anyhow::Result<()> {
        use crate::db::transactions;

        let form = self.txn.form.as_ref().unwrap();

        // If we're in confirmation phase (installment create)
        if let Some(conf) = &form.confirmation {
            let confirmed = conf.confirmed;
            if confirmed {
                let validated = self
                    .txn.form
                    .as_mut()
                    .unwrap()
                    .confirmation
                    .take()
                    .unwrap()
                    .validated;

                db::installments::create_installment_purchase(
                    &self.pool,
                    validated.total_amount,
                    validated.installment_count,
                    &validated.description,
                    validated.category_id,
                    validated.account_id,
                    validated.first_date,
                )
                .await?;
                info!(
                    desc = %validated.description,
                    total = %validated.total_amount,
                    count = validated.installment_count,
                    "installment purchase created"
                );

                self.txn.form = None;
                self.input_mode = InputMode::Normal;
                self.refresh_installments().await?;
                self.load_transactions().await?;
                self.refresh_balances().await?;
                self.refresh_budgets().await?;
                self.refresh_dashboard_statements().await?;
            } else {
                // No selected — return to editing
                self.txn.form.as_mut().unwrap().confirmation = None;
            }
            return Ok(());
        }

        let validated = match form.validate(&self.accounts, &self.categories, self.locale) {
            Ok(v) => v,
            Err(e) => {
                self.txn.form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        // If this is an installment create, validate count and show confirmation
        if form.is_installment {
            let count = match form.validate_installment_count(self.locale) {
                Ok(c) => c,
                Err(e) => {
                    self.txn.form.as_mut().unwrap().error = Some(e);
                    return Ok(());
                }
            };

            let per_installment =
                (validated.amount / Decimal::from(count)).round_dp(2);

            let account = self
                .accounts
                .iter()
                .find(|a| a.id == validated.account_id)
                .unwrap();
            let billing_day = account.billing_day.unwrap_or(1) as u32;
            let due_day = account.due_day.unwrap_or(1) as u32;

            let mut parcela_dues = Vec::with_capacity(count as usize);
            for i in 1..=count {
                let date = db::add_months(validated.date, (i - 1) as u32);
                let close = latest_closing_date(date, billing_day);
                let statement_close = if date > close {
                    let (ny, nm) = next_month(close.year(), close.month());
                    clamped_day(ny, nm, billing_day)
                } else {
                    close
                };
                let due = statement_due_date(
                    statement_close.year(),
                    statement_close.month(),
                    billing_day,
                    due_day,
                );
                parcela_dues.push((i, due));
            }

            self.txn.form.as_mut().unwrap().confirmation = Some(InstallmentConfirmation {
                validated: ValidatedInstallment {
                    description: validated.description,
                    total_amount: validated.amount,
                    installment_count: count,
                    first_date: validated.date,
                    account_id: validated.account_id,
                    category_id: validated.category_id,
                },
                per_installment,
                parcela_dues,
                confirmed: false,
            });
            return Ok(());
        }

        // Regular transaction create/edit
        let params = transactions::TransactionParams {
            amount: validated.amount,
            description: validated.description,
            category_id: validated.category_id,
            account_id: validated.account_id,
            transaction_type: validated.transaction_type,
            payment_method: validated.payment_method,
            date: validated.date,
        };

        match form.mode {
            TransactionFormMode::Create => {
                transactions::create_transaction(&self.pool, &params).await?;
                info!(
                    desc = %params.description,
                    amount = %params.amount,
                    "transaction created"
                );
            }
            TransactionFormMode::Edit(id) => {
                transactions::update_transaction(&self.pool, id, &params).await?;
                info!(id, desc = %params.description, "transaction updated");
            }
        }

        self.txn.form = None;
        self.input_mode = InputMode::Normal;
        self.load_transactions().await?;
        self.refresh_balances().await?;
        self.refresh_budgets().await?;
        self.refresh_dashboard_statements().await?;
        Ok(())
    }

    /// Two-phase submit for installment edit form.
    pub(crate) async fn submit_installment_form(&mut self) -> anyhow::Result<()> {
        if self.txn.inst_form.as_ref().unwrap().confirmation.is_some() {
            let confirmed = self
                .txn.inst_form
                .as_ref()
                .unwrap()
                .confirmation
                .as_ref()
                .unwrap()
                .confirmed;

            if confirmed {
                let validated = self
                    .txn.inst_form
                    .as_mut()
                    .unwrap()
                    .confirmation
                    .take()
                    .unwrap()
                    .validated;
                let mode = self.txn.inst_form.as_ref().unwrap().mode;

                match mode {
                    InstallmentFormMode::Create => unreachable!(),
                    InstallmentFormMode::Edit(id) => {
                        db::installments::update_installment_purchase(
                            &self.pool,
                            id,
                            validated.total_amount,
                            validated.installment_count,
                            &validated.description,
                            validated.category_id,
                            validated.first_date,
                        )
                        .await?;
                        info!(
                            id,
                            desc = %validated.description,
                            total = %validated.total_amount,
                            count = validated.installment_count,
                            "installment purchase updated"
                        );
                    }
                }

                self.txn.inst_form = None;
                self.input_mode = InputMode::Normal;
                self.refresh_installments().await?;
                self.load_transactions().await?;
                self.refresh_balances().await?;
                self.refresh_budgets().await?;
                self.refresh_dashboard_statements().await?;
            } else {
                // No selected — return to editing
                self.txn.inst_form.as_mut().unwrap().confirmation = None;
            }
            return Ok(());
        }

        // Validate and show confirmation
        let form = self.txn.inst_form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts, &self.categories, self.locale) {
            Ok(v) => v,
            Err(e) => {
                self.txn.inst_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        let per_installment =
            (validated.total_amount / Decimal::from(validated.installment_count)).round_dp(2);

        let account = self
            .accounts
            .iter()
            .find(|a| a.id == validated.account_id)
            .unwrap();
        let billing_day = account.billing_day.unwrap_or(1) as u32;
        let due_day = account.due_day.unwrap_or(1) as u32;

        let mut parcela_dues = Vec::with_capacity(validated.installment_count as usize);
        for i in 1..=validated.installment_count {
            let date = db::add_months(validated.first_date, (i - 1) as u32);
            let close = latest_closing_date(date, billing_day);
            let statement_close = if date > close {
                let (ny, nm) = next_month(close.year(), close.month());
                clamped_day(ny, nm, billing_day)
            } else {
                close
            };
            let due = statement_due_date(
                statement_close.year(),
                statement_close.month(),
                billing_day,
                due_day,
            );
            parcela_dues.push((i, due));
        }

        self.txn.inst_form.as_mut().unwrap().confirmation = Some(InstallmentConfirmation {
            validated,
            per_installment,
            parcela_dues,
            confirmed: false,
        });

        Ok(())
    }

    pub(crate) async fn execute_delete_transaction(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::transactions::delete_transaction(&self.pool, id).await?;
        info!(id, "transaction deleted");
        self.load_transactions().await?;
        self.refresh_balances().await?;
        self.refresh_budgets().await?;
        self.refresh_dashboard_statements().await?;
        clamp_selection(&mut self.txn.table_state, self.txn.items.len());
        Ok(())
    }

    pub(crate) async fn execute_delete_installment(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::installments::delete_installment_purchase(&self.pool, id).await?;
        info!(id, "installment purchase deleted (transactions cascaded)");
        self.refresh_installments().await?;
        self.load_transactions().await?;
        self.refresh_balances().await?;
        self.refresh_budgets().await?;
        self.refresh_dashboard_statements().await?;
        clamp_selection(&mut self.txn.table_state, self.txn.items.len());
        Ok(())
    }
}
