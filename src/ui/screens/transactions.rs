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
    models::{Account, Category, CategoryType, PaymentMethod, Transaction, TransactionType},
    ui::{
        App,
        components::{format::format_brl, input::InputField, toggle::render_toggle},
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

        let amount = self
            .amount
            .value
            .trim()
            .replace(',', ".")
            .parse::<Decimal>()
            .map_err(|_| "Invalid amount".to_string())
            .and_then(|v| {
                if v > Decimal::ZERO {
                    Ok(v)
                } else {
                    Err("Amount must be positive".into())
                }
            })?;

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
            .filter(|c| c.parsed_type() == category_type_for(transaction_type))
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

    pub fn new_create(accounts: &[Account], categories: &[Category]) -> Self {
        let today = Local::now().date_naive();
        let default_type = TransactionType::Expense;

        Self {
            mode: TransactionFormMode::Create,
            date: InputField::new("Date").with_value(today.format("%d-%m-%Y").to_string()),
            description: InputField::new("Description"),
            amount: InputField::new("Amount"),
            transaction_type: default_type,
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
            .filter(|c| c.parsed_type() == category_type_for(txn_type))
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
    let [table_area, detail_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);

    let header = Row::new(["Date", "Description", "Amount", "Account", "Category"])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .transactions
        .iter()
        .map(|txn| {
            let account_name = app
                .accounts
                .iter()
                .find(|a| a.id == txn.account_id)
                .map(|a| a.name.as_str())
                .unwrap_or("?");
            let category_name = app
                .categories
                .iter()
                .find(|c| c.id == txn.category_id)
                .map(|c| c.name.as_str())
                .unwrap_or("?");

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
    .block(Block::default().borders(Borders::ALL).title("Transactions"))
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
            let account_name = app
                .accounts
                .iter()
                .find(|a| a.id == txn.account_id)
                .map(|a| a.name.as_str())
                .unwrap_or("?");

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
                if names.is_empty() {
                    Line::from(Span::styled(
                        " Account: (no accounts)",
                        Style::new().fg(Color::DarkGray),
                    ))
                } else {
                    render_toggle("Account", &names, form.account_idx, active)
                }
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
                if methods.is_empty() {
                    Line::from(Span::styled(
                        " Payment: (none)",
                        Style::new().fg(Color::DarkGray),
                    ))
                } else {
                    render_toggle("Payment", &methods, form.payment_method_idx, active)
                }
            }
            TransactionField::Category => {
                let filtered: Vec<&str> = app
                    .categories
                    .iter()
                    .filter(|c| c.parsed_type() == category_type_for(form.transaction_type))
                    .map(|c| c.name.as_str())
                    .collect();
                if filtered.is_empty() {
                    Line::from(Span::styled(
                        " Category: (none)",
                        Style::new().fg(Color::DarkGray),
                    ))
                } else {
                    render_toggle("Category", &filtered, form.category_idx, active)
                }
            }
        };
        lines.push(line);
    }

    if let Some(err) = &form.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Error: {}", err),
            Style::new().fg(Color::Red),
        )));
    }

    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

pub(crate) fn category_type_for(txn_type: TransactionType) -> CategoryType {
    match txn_type {
        TransactionType::Expense => CategoryType::Expense,
        TransactionType::Income => CategoryType::Income,
    }
}
