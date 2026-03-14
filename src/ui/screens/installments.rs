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
    models::{Category, CategoryType},
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
    pub description: InputField,
    pub total_amount: InputField,
    pub installment_count: InputField,
    pub first_date: InputField,
    pub account_idx: usize,
    pub category_idx: usize,
    pub active_field: usize,
    pub error: Option<String>,
}

pub struct ValidatedInstallment {
    pub description: String,
    pub total_amount: Decimal,
    pub installment_count: i16,
    pub first_date: NaiveDate,
    pub account_id: i32,
    pub category_id: i32,
}

impl InstallmentForm {
    pub fn validate(
        &self,
        accounts: &[crate::models::Account],
        categories: &[Category],
    ) -> Result<ValidatedInstallment, String> {
        let description = self.description.value.trim().to_string();
        if description.is_empty() {
            return Err("Description is required".into());
        }

        let total_amount = parse_positive_amount(&self.total_amount.value)?;

        let installment_count = self
            .installment_count
            .value
            .trim()
            .parse::<i16>()
            .map_err(|_| "Invalid installment count".to_string())
            .and_then(|v| {
                if v >= 2 {
                    Ok(v)
                } else {
                    Err("Must be at least 2 installments".into())
                }
            })?;

        let first_date = NaiveDate::parse_from_str(self.first_date.value.trim(), "%d-%m-%Y")
            .map_err(|_| "Invalid date (use DD-MM-YYYY)".to_string())?;

        // Only accounts with credit card
        let credit_accounts: Vec<&crate::models::Account> =
            accounts.iter().filter(|a| a.has_credit_card).collect();
        let account_id = credit_accounts
            .get(self.account_idx)
            .map(|a| a.id)
            .ok_or("No account selected")?;

        let expense_categories: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parsed_type() == CategoryType::Expense)
            .collect();
        let category_id = expense_categories
            .get(self.category_idx)
            .map(|c| c.id)
            .ok_or("No category selected")?;

        Ok(ValidatedInstallment {
            description,
            total_amount,
            installment_count,
            first_date,
            account_id,
            category_id,
        })
    }

    pub fn new_create() -> Self {
        let today = Local::now().date_naive();
        Self {
            description: InputField::new("Description"),
            total_amount: InputField::new("Total Amount"),
            installment_count: InputField::new("Installments"),
            first_date: InputField::new("First Date").with_value(today.format("%d-%m-%Y").to_string()),
            account_idx: 0,
            category_idx: 0,
            active_field: 0,
            error: None,
        }
    }

    pub fn active_field_id(&self) -> InstallmentField {
        InstallmentField::ALL[self.active_field.min(InstallmentField::ALL.len() - 1)]
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.installment_form.is_some() {
        render_form(frame, area, app);
    } else {
        render_list(frame, area, app);
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let [table_area, detail_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);

    let header = Row::new(["Description", "Total", "# Inst.", "First Date", "Account", "Category"])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .installments
        .iter()
        .map(|ip| {
            let account_name = app.account_name(ip.account_id);
            let category_name = app.category_name(ip.category_id);

            Row::new([
                ip.description.clone(),
                format_brl(ip.total_amount),
                ip.installment_count.to_string(),
                ip.first_installment_date.format("%d-%m-%Y").to_string(),
                account_name.to_string(),
                category_name.to_string(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(15),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Length(14),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Installment Purchases"))
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.installment_table_state);

    let detail_content = match app
        .installment_table_state
        .selected()
        .and_then(|i| app.installments.get(i))
    {
        Some(ip) => {
            let account_name = app.account_name(ip.account_id);
            let category_name = app.category_name(ip.category_id);

            let per_installment = ip.total_amount / Decimal::from(ip.installment_count);

            vec![
                Line::from(format!(
                    " {} | {} | {}",
                    ip.description, account_name, category_name,
                )),
                Line::from(format!(
                    " Total: {} | ~{}/installment | {} installments",
                    format_brl(ip.total_amount),
                    format_brl(per_installment.round_dp(2)),
                    ip.installment_count,
                )),
                Line::from(format!(
                    " First: {} | Created: {}",
                    ip.first_installment_date.format("%d-%m-%Y"),
                    ip.created_at.format("%d-%m-%Y %H:%M"),
                )),
            ]
        }
        None => vec![Line::from(" No installment purchase selected.")],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("Installment Details");
    let detail = Paragraph::new(detail_content).block(detail_block);
    frame.render_widget(detail, detail_area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.installment_form.as_ref().unwrap();

    let title = "New Installment Purchase";

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
                render_selector("Account", &names, form.account_idx, active, "no accounts with credit card")
            }
            InstallmentField::Category => {
                let names: Vec<&str> = app
                    .categories
                    .iter()
                    .filter(|c| c.parsed_type() == CategoryType::Expense)
                    .map(|c| c.name.as_str())
                    .collect();
                render_selector("Category", &names, form.category_idx, active, "no expense categories")
            }
        };
        lines.push(line);
    }

    push_form_error(&mut lines, &form.error);

    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
