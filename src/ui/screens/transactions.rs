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
use tracing::{debug, info};

use crate::{
    db::transactions::TransactionFilterParams,
    models::{Account, Category, PaymentMethod, Transaction, TransactionType},
    ui::{
        App,
        app::{
            ConfirmAction, InputMode, StatusMessage, PAGE_SIZE, clamp_selection, cycle_index,
            is_toggle_key, move_table_selection,
        },
        components::{
            format::{format_brl, parse_positive_amount},
            input::InputField,
            toggle::{push_form_error, render_selector, render_toggle},
        },
    },
};

pub enum TransactionFormMode {
    Create,
    Edit(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionField {
    Date,
    Description,
    Amount,
    TransactionType,
    Account,
    PaymentMethod,
    Category,
}

impl TransactionField {
    pub const ALL: [Self; 7] = [
        Self::Date,
        Self::Description,
        Self::Amount,
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
    pub fn new() -> Self {
        Self {
            date_from: InputField::new("From"),
            date_to: InputField::new("To"),
            description: InputField::new("Desc"),
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
        Self::new()
    }
}

/// Cycle through `None → Some(0) → … → Some(len-1) → None`.
/// Used by filter selectors so "All" (None) is reachable by cycling past the last option.
pub fn cycle_option(current: Option<usize>, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match current {
        None => Some(0),
        Some(i) if i + 1 < len => Some(i + 1),
        Some(_) => None,
    }
}

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
    pub fn validate(
        &self,
        accounts: &[Account],
        categories: &[Category],
    ) -> Result<ValidatedTransaction, String> {
        let date = NaiveDate::parse_from_str(self.date.value.trim(), "%d-%m-%Y")
            .map_err(|_| "Invalid date (use DD-MM-YYYY)".to_string())?;

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

    pub fn new_create() -> Self {
        let today = Local::now().date_naive();

        Self {
            mode: TransactionFormMode::Create,
            date: InputField::new("Date").with_value(today.format("%d-%m-%Y").to_string()),
            description: InputField::new("Description"),
            amount: InputField::new("Amount"),
            transaction_type: TransactionType::Expense,
            account_idx: 0,
            payment_method_idx: 0,
            category_idx: 0,
            active_field: 0,
            error: None,
        }
    }

    pub fn new_edit(txn: &Transaction, accounts: &[Account], categories: &[Category]) -> Self {
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
            date: InputField::new("Date").with_value(txn.date.format("%d-%m-%Y").to_string()),
            description: InputField::new("Description").with_value(&txn.description),
            amount: InputField::new("Amount").with_value(txn.amount.to_string()),
            transaction_type: txn_type,
            account_idx,
            payment_method_idx,
            category_idx,
            active_field: 0,
            error: None,
        }
    }

    pub fn active_field_id(&self) -> TransactionField {
        TransactionField::ALL[self.active_field.min(TransactionField::ALL.len() - 1)]
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.transaction_form.is_some() {
        render_form(frame, area, app);
    } else {
        render_list(frame, area, app);
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let filter_height = if app.transaction_filter.visible { 4 } else { 0 };

    let [filter_area, table_area, detail_area] = Layout::vertical([
        Constraint::Length(filter_height),
        Constraint::Min(5),
        Constraint::Length(7),
    ])
    .areas(area);

    if app.transaction_filter.visible {
        render_filter_bar(frame, filter_area, app);
    }

    let header = Row::new(["Date", "Description", "Amount", "Account", "Category"])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .transactions
        .iter()
        .map(|txn| {
            let account_name = app.account_name(txn.account_id);
            let category_name = app.category_name(txn.category_id);

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
    .block(Block::default().borders(Borders::ALL).title(if app.transaction_count > 0 {
        let start = app.transaction_offset + 1;
        let end = app.transaction_offset + app.transactions.len() as u64;
        format!("Transactions ({start}-{end} of {})", app.transaction_count)
    } else {
        "Transactions (0)".to_string()
    }))
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.transaction_table_state);

    let detail_content = match app
        .transaction_table_state
        .selected()
        .and_then(|i| app.transactions.get(i))
    {
        Some(txn) => {
            let account_name = app.account_name(txn.account_id);

            let installment_info = match (txn.installment_purchase_id, txn.installment_number) {
                (Some(_), Some(n)) => format!(" | Installment #{n}"),
                _ => String::new(),
            };

            vec![
                Line::from(format!(
                    " {} | {} | {}",
                    txn.date.format("%d-%m-%Y"),
                    txn.parsed_type().label(),
                    txn.parsed_payment_method().label(),
                )),
                Line::from(format!(
                    " {} | {} | {}{}",
                    txn.description,
                    format_brl(txn.amount),
                    account_name,
                    installment_info,
                )),
                Line::from(format!(
                    " Created: {}",
                    txn.created_at.format("%d-%m-%Y %H:%M")
                )),
                Line::from(Span::styled(
                    " [n] New  [e] Edit  [d] Delete  [f] Filter  [r] Reset  [x] Export",
                    Style::new().fg(Color::DarkGray),
                )),
            ]
        }
        None => vec![Line::from(" No transaction selected.")],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("Transaction Details");
    let detail = Paragraph::new(detail_content).block(detail_block);
    frame.render_widget(detail, detail_area);
}

fn render_filter_bar(frame: &mut Frame, area: Rect, app: &mut App) {
    let filter = &app.transaction_filter;
    let is_filtering = app.input_mode == InputMode::Filtering;
    let border_color = if is_filtering { Color::Yellow } else { Color::DarkGray };

    // Row 1: DateFrom | DateTo | Description (text inputs)
    let mut row1: Vec<Span> = Vec::new();
    for (i, field) in [FilterField::DateFrom, FilterField::DateTo, FilterField::Description]
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

        let (label, value) = match field {
            FilterField::Account => (
                "Acct",
                filter
                    .account_idx
                    .and_then(|i| app.accounts.get(i))
                    .map(|a| a.name.as_str())
                    .unwrap_or("All"),
            ),
            FilterField::Category => (
                "Cat",
                filter
                    .category_idx
                    .and_then(|i| app.categories.get(i))
                    .map(|c| c.name.as_str())
                    .unwrap_or("All"),
            ),
            FilterField::TransactionType => (
                "Type",
                match filter.transaction_type_idx {
                    Some(0) => "Expense",
                    Some(_) => "Income",
                    None => "All",
                },
            ),
            FilterField::PaymentMethod => {
                const LABELS: [&str; 6] =
                    ["PIX", "Credit Card", "Debit Card", "Cash", "Boleto", "Transfer"];
                (
                    "Pay",
                    filter
                        .payment_method_idx
                        .and_then(|i| LABELS.get(i).copied())
                        .unwrap_or("All"),
                )
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
        .title("Filter (Enter=apply, Esc=close, r=reset)")
        .border_style(Style::new().fg(border_color));

    let lines = vec![Line::from(row1), Line::from(row2)];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.transaction_form.as_ref().unwrap();

    let title = match form.mode {
        TransactionFormMode::Create => "New Transaction",
        TransactionFormMode::Edit(_) => "Edit Transaction",
    };

    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in TransactionField::ALL.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            TransactionField::Date => form.date.render_line(active),
            TransactionField::Description => form.description.render_line(active),
            TransactionField::Amount => form.amount.render_line(active),
            TransactionField::TransactionType => render_toggle(
                "Type",
                &["Expense", "Income"],
                if form.transaction_type == TransactionType::Expense {
                    0
                } else {
                    1
                },
                active,
            ),
            TransactionField::Account => {
                let names: Vec<&str> = app.accounts.iter().map(|a| a.name.as_str()).collect();
                render_selector("Account", &names, form.account_idx, active, "no accounts")
            }
            TransactionField::PaymentMethod => {
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
            TransactionField::Category => {
                let filtered: Vec<&str> = app
                    .categories
                    .iter()
                    .filter(|c| c.parsed_type() == form.transaction_type.category_type())
                    .map(|c| c.name.as_str())
                    .collect();
                render_selector("Category", &filtered, form.category_idx, active, "none")
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
                            Some(crate::ui::components::popup::ConfirmPopup::new(format!(
                                "Delete \"{txn_desc}\"?"
                            )));
                    }
                }
            }
            KeyCode::Char('f') => {
                self.transaction_filter.visible = true;
                self.transaction_filter.active_field = 0;
                self.input_mode = InputMode::Filtering;
            }
            KeyCode::Char('r') => {
                self.transaction_filter = TransactionFilter::new();
                self.transaction_offset = 0;
                self.load_transactions().await?;
                self.transaction_table_state.select(Some(0));
            }
            KeyCode::Char('x') => {
                let params = self
                    .transaction_filter
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
                                self.status_message = Some(StatusMessage::info(format!(
                                    "Exported {} transactions to {}",
                                    all_txns.len(),
                                    path.display()
                                )));
                            }
                            Err(e) => {
                                self.status_message =
                                    Some(StatusMessage::error(format!("Export failed: {e}")));
                            }
                        }
                    }
                    Err(e) => {
                        self.status_message =
                            Some(StatusMessage::error(format!("Export failed: {e}")));
                    }
                }
            }
            KeyCode::PageUp => {
                if self.transaction_offset > 0 {
                    self.transaction_offset =
                        self.transaction_offset.saturating_sub(PAGE_SIZE);
                    self.load_transactions().await?;
                    self.transaction_table_state.select(Some(0));
                }
            }
            KeyCode::PageDown => {
                let next = self.transaction_offset + PAGE_SIZE;
                if next < self.transaction_count {
                    self.transaction_offset = next;
                    self.load_transactions().await?;
                    self.transaction_table_state.select(Some(0));
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
                self.transaction_offset = 0;
                self.load_transactions().await?;
                self.transaction_table_state.select(Some(0));
                self.input_mode = InputMode::Normal;
                debug!(count = self.transaction_count, "filters applied");
            }
            KeyCode::Tab | KeyCode::Down => {
                if self.transaction_filter.active_field < FilterField::ALL.len() - 1 {
                    self.transaction_filter.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if self.transaction_filter.active_field > 0 {
                    self.transaction_filter.active_field -= 1;
                }
            }
            _ => match self.transaction_filter.active_field_id() {
                FilterField::DateFrom => {
                    self.transaction_filter.date_from.handle_key(key.code);
                }
                FilterField::DateTo => {
                    self.transaction_filter.date_to.handle_key(key.code);
                }
                FilterField::Description => {
                    self.transaction_filter.description.handle_key(key.code);
                }
                FilterField::Account => {
                    if is_toggle_key(key.code) {
                        let len = self.accounts.len();
                        self.transaction_filter.account_idx =
                            cycle_option(self.transaction_filter.account_idx, len);
                    }
                }
                FilterField::Category => {
                    if is_toggle_key(key.code) {
                        let len = self.categories.len();
                        self.transaction_filter.category_idx =
                            cycle_option(self.transaction_filter.category_idx, len);
                    }
                }
                FilterField::TransactionType => {
                    if is_toggle_key(key.code) {
                        self.transaction_filter.transaction_type_idx =
                            cycle_option(self.transaction_filter.transaction_type_idx, 2);
                    }
                }
                FilterField::PaymentMethod => {
                    if is_toggle_key(key.code) {
                        self.transaction_filter.payment_method_idx =
                            cycle_option(self.transaction_filter.payment_method_idx, 6);
                    }
                }
            },
        }
        Ok(())
    }

    pub(crate) fn handle_transaction_form_key(&mut self, code: KeyCode) {
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

    pub(crate) async fn submit_transaction_form(&mut self) -> anyhow::Result<()> {
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
                info!(
                    desc = %validated.description,
                    amount = %validated.amount,
                    "transaction created"
                );
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
                info!(id, desc = %validated.description, "transaction updated");
            }
        }

        self.transaction_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    pub(crate) async fn execute_delete_transaction(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::transactions::delete_transaction(&self.pool, id).await?;
        info!(id, "transaction deleted");
        self.load_data().await?;
        clamp_selection(&mut self.transaction_table_state, self.transactions.len());
        Ok(())
    }
}
