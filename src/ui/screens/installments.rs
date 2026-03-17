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
    models::{Category, CategoryType},
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
        Layout::vertical([Constraint::Min(5), Constraint::Length(7)]).areas(area);

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
                Line::from(Span::styled(
                    " [n] New  [d] Delete  [x] Export",
                    Style::new().fg(Color::DarkGray),
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

// ── Key handling & form submission (impl App) ─────────────────────

impl App {
    pub(crate) async fn handle_installments_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up => {
                move_table_selection(
                    &mut self.installment_table_state,
                    self.installments.len(),
                    -1,
                );
            }
            KeyCode::Down => {
                move_table_selection(
                    &mut self.installment_table_state,
                    self.installments.len(),
                    1,
                );
            }
            KeyCode::Char('n') => {
                let has_credit_account = self.accounts.iter().any(|a| a.has_credit_card);
                let has_expense_cat = self
                    .categories
                    .iter()
                    .any(|c| c.parsed_type() == CategoryType::Expense);
                if !has_credit_account {
                    self.status_message = Some(StatusMessage::error(
                        "No account with credit card available",
                    ));
                } else if !has_expense_cat {
                    self.status_message =
                        Some(StatusMessage::error("Create an expense category first"));
                } else {
                    self.installment_form = Some(InstallmentForm::new_create());
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(ip) = self
                    .installment_table_state
                    .selected()
                    .and_then(|i| self.installments.get(i))
                {
                    let ip_id = ip.id;
                    let ip_desc = ip.description.clone();
                    self.confirm_action = Some(ConfirmAction::DeleteInstallment(ip_id));
                    self.confirm_popup = Some(ConfirmPopup::new(format!(
                        "Delete \"{}\" and all its transactions?",
                        ip_desc
                    )));
                }
            }
            KeyCode::Char('x') => {
                let acct_names = &self.account_names;
                let cats = &self.categories;
                match crate::export::export_installments(
                    &self.installments,
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
                            "Exported {} installments to {}",
                            self.installments.len(),
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

    pub(crate) fn handle_installment_form_key(&mut self, code: KeyCode) {
        let form = self.installment_form.as_mut().unwrap();
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
                    if is_toggle_key(code) {
                        let len = self.accounts.iter().filter(|a| a.has_credit_card).count();
                        cycle_index(&mut form.account_idx, len, code);
                    }
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

    pub(crate) async fn submit_installment_form(&mut self) -> anyhow::Result<()> {
        use crate::db::installments;

        let form = self.installment_form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts, &self.categories) {
            Ok(v) => v,
            Err(e) => {
                self.installment_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        installments::create_installment_purchase(
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

        self.installment_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    pub(crate) async fn execute_delete_installment(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::installments::delete_installment_purchase(&self.pool, id).await?;
        info!(id, "installment purchase deleted (transactions cascaded)");
        self.load_data().await?;
        clamp_selection(&mut self.installment_table_state, self.installments.len());
        Ok(())
    }
}
