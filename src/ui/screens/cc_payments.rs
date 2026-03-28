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
        i18n::{Locale, t, tf_paginated},
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
    pub fn new_create(locale: Locale) -> Self {
        let today = Local::now().date_naive();

        Self {
            date: InputField::new(t(locale, "form.date")).with_value(today.format("%d-%m-%Y").to_string()),
            account_idx: 0,
            amount: InputField::new(t(locale, "form.amount")),
            description: InputField::new(t(locale, "form.description")),
            active_field: 0,
            error: None,
        }
    }

    pub fn validate(&self, accounts: &[Account], locale: Locale) -> Result<ValidatedCcPayment, String> {
        let date = NaiveDate::parse_from_str(self.date.value.trim(), "%d-%m-%Y")
            .map_err(|_| t(locale, "err.invalid_date").to_string())?;

        let cc_accounts: Vec<&Account> = accounts.iter().filter(|a| a.has_credit_card).collect();

        let account = cc_accounts
            .get(self.account_idx)
            .ok_or(t(locale, "err.no_account"))?;

        let amount = parse_positive_amount(&self.amount.value, locale)?;

        let description = self.description.value.trim().to_string();
        if description.is_empty() {
            return Err(t(locale, "err.description_required").into());
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
    if app.cc_pay.form.is_some() {
        render_form(frame, area, app);
    } else {
        render_list(frame, area, app);
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let [table_area, detail_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);

    let header = Row::new([
        t(app.locale, "header.date"),
        t(app.locale, "header.account"),
        t(app.locale, "header.amount"),
        t(app.locale, "header.description"),
    ])
    .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .cc_pay.items
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
            .title(if app.cc_pay.count > 0 {
                let start = app.cc_pay.offset + 1;
                let end = app.cc_pay.offset + app.cc_pay.items.len() as u64;
                tf_paginated(app.locale, t(app.locale, "title.cc_payments"), start, end, app.cc_pay.count)
            } else {
                format!("{} (0)", t(app.locale, "title.cc_payments"))
            }),
    )
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.cc_pay.table_state);

    let detail_content = match app
        .cc_pay.table_state
        .selected()
        .and_then(|i| app.cc_pay.items.get(i))
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
                    " {}: {}",
                    t(app.locale, "detail.created"),
                    p.created_at.format("%d-%m-%Y %H:%M")
                )),
                Line::from(Span::styled(
                    format!(" {}", t(app.locale, "hint.cc_pay")),
                    Style::new().fg(Color::DarkGray),
                )),
            ]
        }
        None => vec![Line::from(format!(" {}", t(app.locale, "misc.no_sel.payment")))],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title(t(app.locale, "title.payment_details"));
    let detail = Paragraph::new(detail_content).block(detail_block);
    frame.render_widget(detail, detail_area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.cc_pay.form.as_ref().unwrap();

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
                    t(app.locale, "form.account"),
                    &names,
                    form.account_idx,
                    active,
                    t(app.locale, "misc.no_cc_accounts"),
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
        .title(t(app.locale, "title.new_cc_payment"));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

// ── Key handling & form submission (impl App) ─────────────────────

impl App {
    pub(crate) async fn handle_cc_payments_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_selection(&mut self.cc_pay.table_state, self.cc_pay.items.len(), -1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_selection(&mut self.cc_pay.table_state, self.cc_pay.items.len(), 1);
            }
            KeyCode::Char('n') => {
                let has_cc = self.accounts.iter().any(|a| a.has_credit_card);
                if !has_cc {
                    self.status_message = Some(StatusMessage::error(
                        t(self.locale, "msg.no_cc_account"),
                    ));
                } else {
                    self.cc_pay.form = Some(CcPaymentForm::new_create(self.locale));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(p) = self
                    .cc_pay.table_state
                    .selected()
                    .and_then(|i| self.cc_pay.items.get(i))
                {
                    let p_id = p.id;
                    let p_desc = p.description.clone();
                    self.confirm_action = Some(ConfirmAction::DeleteCreditCardPayment(p_id));
                    self.confirm_popup = Some(ConfirmPopup::new(
                        crate::ui::i18n::tf_delete_payment(self.locale, &p_desc)
                    ));
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
                                self.status_message = Some(StatusMessage::info(
                                    crate::ui::i18n::tf_exported(self.locale, all_payments.len(), t(self.locale, "export.payments"), &path)
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
                if self.cc_pay.offset > 0 {
                    self.cc_pay.offset = self.cc_pay.offset.saturating_sub(PAGE_SIZE);
                    self.load_cc_payments().await?;
                    self.cc_pay.table_state.select(Some(0));
                }
            }
            KeyCode::PageDown => {
                let next = self.cc_pay.offset + PAGE_SIZE;
                if next < self.cc_pay.count {
                    self.cc_pay.offset = next;
                    self.load_cc_payments().await?;
                    self.cc_pay.table_state.select(Some(0));
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_cc_payment_form_key(&mut self, code: KeyCode) {
        let form = self.cc_pay.form.as_mut().unwrap();
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

        let form = self.cc_pay.form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts, self.locale) {
            Ok(v) => v,
            Err(e) => {
                self.cc_pay.form.as_mut().unwrap().error = Some(e);
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

        self.cc_pay.form = None;
        self.input_mode = InputMode::Normal;
        self.load_cc_payments().await?;
        self.refresh_balances().await?;
        self.refresh_dashboard_statements().await?;
        Ok(())
    }

    pub(crate) async fn execute_delete_cc_payment(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::credit_card_payments::delete_payment(&self.pool, id).await?;
        info!(id, "credit card payment deleted");
        self.load_cc_payments().await?;
        self.refresh_balances().await?;
        self.refresh_dashboard_statements().await?;
        clamp_selection(&mut self.cc_pay.table_state, self.cc_pay.items.len());
        Ok(())
    }
}
