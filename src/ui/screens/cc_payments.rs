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
    models::Account,
    ui::{
        App,
        app::{
            ConfirmAction, InputMode, PAGE_SIZE, StatusMessage, clamp_selection, cycle_index,
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
pub enum CcPaymentField {
    Date,
    Account,
    Amount,
    Description,
}

impl CcPaymentField {
    pub const ALL: [Self; 4] = [Self::Date, Self::Account, Self::Amount, Self::Description];
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

        let cc_accounts: Vec<&Account> = accounts.iter().filter(|a| a.has_credit_card).collect();

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
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);

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
            .title(if app.cc_payment_count > 0 {
                let start = app.cc_payment_offset + 1;
                let end = app.cc_payment_offset + app.cc_payments.len() as u64;
                format!(
                    "Credit Card Payments ({start}-{end} of {})",
                    app.cc_payment_count
                )
            } else {
                "Credit Card Payments (0)".to_string()
            }),
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
                Line::from(Span::styled(
                    " [n] New  [d] Delete  [x] Export",
                    Style::new().fg(Color::DarkGray),
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
                render_selector(
                    "Account",
                    &names,
                    form.account_idx,
                    active,
                    "no credit card accounts",
                )
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

// ── Key handling & form submission (impl App) ─────────────────────

impl App {
    pub(crate) async fn handle_cc_payments_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_selection(&mut self.cc_payment_table_state, self.cc_payments.len(), -1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_selection(&mut self.cc_payment_table_state, self.cc_payments.len(), 1);
            }
            KeyCode::Char('n') => {
                let has_cc = self.accounts.iter().any(|a| a.has_credit_card);
                if !has_cc {
                    self.status_message = Some(StatusMessage::error(
                        "No account with credit card available",
                    ));
                } else {
                    self.cc_payment_form = Some(CcPaymentForm::new_create());
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(p) = self
                    .cc_payment_table_state
                    .selected()
                    .and_then(|i| self.cc_payments.get(i))
                {
                    let p_id = p.id;
                    let p_desc = p.description.clone();
                    self.confirm_action = Some(ConfirmAction::DeleteCreditCardPayment(p_id));
                    self.confirm_popup =
                        Some(ConfirmPopup::new(format!("Delete payment \"{}\"?", p_desc)));
                }
            }
            KeyCode::Char('x') => {
                match crate::db::credit_card_payments::list_all_cc_payments(&self.pool).await {
                    Ok(all_payments) => {
                        let acct_names = &self.account_names;
                        match crate::export::export_cc_payments(&all_payments, |id| {
                            acct_names.get(&id).cloned().unwrap_or_else(|| "?".into())
                        }) {
                            Ok(path) => {
                                self.status_message = Some(StatusMessage::info(format!(
                                    "Exported {} payments to {}",
                                    all_payments.len(),
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
                if self.cc_payment_offset > 0 {
                    self.cc_payment_offset = self.cc_payment_offset.saturating_sub(PAGE_SIZE);
                    self.load_cc_payments().await?;
                    self.cc_payment_table_state.select(Some(0));
                }
            }
            KeyCode::PageDown => {
                let next = self.cc_payment_offset + PAGE_SIZE;
                if next < self.cc_payment_count {
                    self.cc_payment_offset = next;
                    self.load_cc_payments().await?;
                    self.cc_payment_table_state.select(Some(0));
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_cc_payment_form_key(&mut self, code: KeyCode) {
        let form = self.cc_payment_form.as_mut().unwrap();
        match code {
            KeyCode::Tab | KeyCode::Down => {
                if form.active_field < CcPaymentField::ALL.len() - 1 {
                    form.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if form.active_field > 0 {
                    form.active_field -= 1;
                }
            }
            _ => match form.active_field_id() {
                CcPaymentField::Date => form.date.handle_key(code),
                CcPaymentField::Account => {
                    if crate::ui::app::is_toggle_key(code) {
                        let len = self.accounts.iter().filter(|a| a.has_credit_card).count();
                        cycle_index(&mut form.account_idx, len, code);
                    }
                }
                CcPaymentField::Amount => form.amount.handle_key(code),
                CcPaymentField::Description => form.description.handle_key(code),
            },
        }
    }

    pub(crate) async fn submit_cc_payment_form(&mut self) -> anyhow::Result<()> {
        use crate::db::credit_card_payments;

        let form = self.cc_payment_form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts) {
            Ok(v) => v,
            Err(e) => {
                self.cc_payment_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        credit_card_payments::create_payment(
            &self.pool,
            validated.account_id,
            validated.amount,
            validated.date,
            &validated.description,
        )
        .await?;
        info!(
            desc = %validated.description,
            amount = %validated.amount,
            "credit card payment created"
        );

        self.cc_payment_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    pub(crate) async fn execute_delete_cc_payment(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::credit_card_payments::delete_payment(&self.pool, id).await?;
        info!(id, "credit card payment deleted");
        self.load_data().await?;
        clamp_selection(&mut self.cc_payment_table_state, self.cc_payments.len());
        Ok(())
    }
}
