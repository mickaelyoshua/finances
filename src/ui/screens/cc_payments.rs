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
pub enum CcPaymentField {
    Date,
    Account,
    Amount,
    Description,
}

impl CcPaymentField {
    pub const ALL: [Self; 4] = [
        Self::Date,
        Self::Account,
        Self::Amount,
        Self::Description,
    ];
}

pub struct CcPaymentForm {
    pub date: InputField,
    pub account_idx: usize,
    pub amount: InputField,
    pub description: InputField,
    pub active_field: usize,
    pub error: Option<String>,
}

pub struct ValidatedCcPayment {
    pub date: NaiveDate,
    pub account_id: i32,
    pub amount: Decimal,
    pub description: String,
}

impl CcPaymentForm {
    pub fn new_create() -> Self {
        let today = Local::now().date_naive();

        Self {
            date: InputField::new("Date").with_value(today.format("%d-%m-%Y").to_string()),
            account_idx: 0,
            amount: InputField::new("Amount"),
            description: InputField::new("Description"),
            active_field: 0,
            error: None,
        }
    }

    pub fn validate(&self, accounts: &[Account]) -> Result<ValidatedCcPayment, String> {
        let date = NaiveDate::parse_from_str(self.date.value.trim(), "%d-%m-%Y")
            .map_err(|_| "Invalid date (use DD-MM-YYYY)".to_string())?;

        let cc_accounts: Vec<&Account> =
            accounts.iter().filter(|a| a.has_credit_card).collect();

        let account = cc_accounts
            .get(self.account_idx)
            .ok_or("No account selected")?;

        let amount = parse_positive_amount(&self.amount.value)?;

        let description = self.description.value.trim().to_string();
        if description.is_empty() {
            return Err("Description is required".into());
        }

        Ok(ValidatedCcPayment {
            date,
            account_id: account.id,
            amount,
            description,
        })
    }

    pub fn active_field_id(&self) -> CcPaymentField {
        CcPaymentField::ALL[self.active_field.min(CcPaymentField::ALL.len() - 1)]
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.cc_payment_form.is_some() {
        render_form(frame, area, app);
    } else {
        render_list(frame, area, app);
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let [table_area, detail_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(5)]).areas(area);

    let header = Row::new(["Date", "Account", "Amount", "Description"])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .cc_payments
        .iter()
        .map(|p| {
            Row::new([
                p.date.format("%d-%m-%Y").to_string(),
                app.account_name(p.account_id).to_string(),
                format_brl(p.amount),
                p.description.clone(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Length(16),
            Constraint::Min(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Credit Card Payments"),
    )
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.cc_payment_table_state);

    let detail_content = match app
        .cc_payment_table_state
        .selected()
        .and_then(|i| app.cc_payments.get(i))
    {
        Some(p) => {
            vec![
                Line::from(format!(
                    " {} | {}",
                    p.date.format("%d-%m-%Y"),
                    app.account_name(p.account_id),
                )),
                Line::from(format!(" {} | {}", format_brl(p.amount), p.description)),
                Line::from(format!(
                    " Created: {}",
                    p.created_at.format("%d-%m-%Y %H:%M")
                )),
            ]
        }
        None => vec![Line::from(" No payment selected.")],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("Payment Details");
    let detail = Paragraph::new(detail_content).block(detail_block);
    frame.render_widget(detail, detail_area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.cc_payment_form.as_ref().unwrap();

    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in CcPaymentField::ALL.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            CcPaymentField::Date => form.date.render_line(active),
            CcPaymentField::Account => {
                let names: Vec<&str> = app
                    .accounts
                    .iter()
                    .filter(|a| a.has_credit_card)
                    .map(|a| a.name.as_str())
                    .collect();
                render_selector("Account", &names, form.account_idx, active, "no credit card accounts")
            }
            CcPaymentField::Amount => form.amount.render_line(active),
            CcPaymentField::Description => form.description.render_line(active),
        };
        lines.push(line);
    }

    push_form_error(&mut lines, &form.error);

    let block = Block::default()
        .borders(Borders::ALL)
        .title("New Credit Card Payment");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
