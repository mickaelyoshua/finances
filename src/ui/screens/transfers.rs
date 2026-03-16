use chrono::{Local, NaiveDate};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use rust_decimal::Decimal;

use crate::{
    models::Account,
    ui::{
        App,
        components::{
            format::{format_brl, parse_positive_amount},
            input::InputField,
            toggle::{push_form_error, render_selector},
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferField {
    Date,
    FromAccount,
    ToAccount,
    Amount,
    Description,
}

impl TransferField {
    pub const ALL: [Self; 5] = [
        Self::Date,
        Self::FromAccount,
        Self::ToAccount,
        Self::Amount,
        Self::Description,
    ];
}

pub struct TransferForm {
    pub date: InputField,
    pub from_account_idx: usize,
    pub to_account_idx: usize,
    pub amount: InputField,
    pub description: InputField,
    pub active_field: usize,
    pub error: Option<String>,
}

pub struct ValidatedTransfer {
    pub date: NaiveDate,
    pub from_account_id: i32,
    pub to_account_id: i32,
    pub amount: Decimal,
    pub description: String,
}

impl TransferForm {
    pub fn new_create() -> Self {
        let today = Local::now().date_naive();

        Self {
            date: InputField::new("Date").with_value(today.format("%d-%m-%Y").to_string()),
            from_account_idx: 0,
            to_account_idx: 1.min(0), // will be clamped by cycle_index
            amount: InputField::new("Amount"),
            description: InputField::new("Description"),
            active_field: 0,
            error: None,
        }
    }

    pub fn validate(&self, accounts: &[Account]) -> Result<ValidatedTransfer, String> {
        let date = NaiveDate::parse_from_str(self.date.value.trim(), "%d-%m-%Y")
            .map_err(|_| "Invalid date (use DD-MM-YYYY)".to_string())?;

        let from_account = accounts
            .get(self.from_account_idx)
            .ok_or("No source account selected")?;
        let to_account = accounts
            .get(self.to_account_idx)
            .ok_or("No destination account selected")?;

        if from_account.id == to_account.id {
            return Err("Source and destination must be different accounts".into());
        }

        let amount = parse_positive_amount(&self.amount.value)?;

        let description = self.description.value.trim().to_string();
        if description.is_empty() {
            return Err("Description is required".into());
        }

        Ok(ValidatedTransfer {
            date,
            from_account_id: from_account.id,
            to_account_id: to_account.id,
            amount,
            description,
        })
    }

    pub fn active_field_id(&self) -> TransferField {
        TransferField::ALL[self.active_field.min(TransferField::ALL.len() - 1)]
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.transfer_form.is_some() {
        render_form(frame, area, app);
    } else {
        render_list(frame, area, app);
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let [table_area, detail_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(5)]).areas(area);

    let header = Row::new(["Date", "From", "To", "Amount", "Description"])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .transfers
        .iter()
        .map(|t| {
            Row::new([
                t.date.format("%d-%m-%Y").to_string(),
                app.account_name(t.from_account_id).to_string(),
                app.account_name(t.to_account_id).to_string(),
                format_brl(t.amount),
                t.description.clone(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(16),
            Constraint::Min(15),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Transfers"))
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.transfer_table_state);

    let detail_content = match app
        .transfer_table_state
        .selected()
        .and_then(|i| app.transfers.get(i))
    {
        Some(t) => {
            vec![
                Line::from(format!(
                    " {} | {} -> {}",
                    t.date.format("%d-%m-%Y"),
                    app.account_name(t.from_account_id),
                    app.account_name(t.to_account_id),
                )),
                Line::from(format!(
                    " {} | {}",
                    format_brl(t.amount),
                    t.description,
                )),
                Line::from(format!(
                    " Created: {}",
                    t.created_at.format("%d-%m-%Y %H:%M")
                )),
            ]
        }
        None => vec![Line::from(" No transfer selected.")],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("Transfer Details");
    let detail = Paragraph::new(detail_content).block(detail_block);
    frame.render_widget(detail, detail_area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.transfer_form.as_ref().unwrap();

    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in TransferField::ALL.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            TransferField::Date => form.date.render_line(active),
            TransferField::FromAccount => {
                let names: Vec<&str> = app.accounts.iter().map(|a| a.name.as_str()).collect();
                render_selector("From", &names, form.from_account_idx, active, "no accounts")
            }
            TransferField::ToAccount => {
                let names: Vec<&str> = app.accounts.iter().map(|a| a.name.as_str()).collect();
                render_selector("To", &names, form.to_account_idx, active, "no accounts")
            }
            TransferField::Amount => form.amount.render_line(active),
            TransferField::Description => form.description.render_line(active),
        };
        lines.push(line);
    }

    push_form_error(&mut lines, &form.error);

    let block = Block::default()
        .borders(Borders::ALL)
        .title("New Transfer");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
