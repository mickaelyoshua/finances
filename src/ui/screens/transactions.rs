use chrono::{Local, NaiveDate};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use rust_decimal::Decimal;

use crate::{
    db::transactions::TransactionFilterParams,
    models::{Account, Category, PaymentMethod, Transaction, TransactionType},
    ui::{
        app::InputMode,
        App,
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
        Constraint::Length(6),
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
