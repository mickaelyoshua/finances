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
    pub fn new_create(locale: Locale) -> Self {
        let today = Local::now().date_naive();

        Self {
            date: InputField::new(t(locale, "form.date")).with_value(today.format("%d-%m-%Y").to_string()),
            from_account_idx: 0,
            to_account_idx: 1, // default to second account; clamped by cycle_index
            amount: InputField::new(t(locale, "form.amount")),
            description: InputField::new(t(locale, "form.description")),
            active_field: 0,
            error: None,
        }
    }

    pub fn validate(&self, accounts: &[Account], locale: Locale) -> Result<ValidatedTransfer, String> {
        let date = NaiveDate::parse_from_str(self.date.value.trim(), "%d-%m-%Y")
            .map_err(|_| t(locale, "err.invalid_date").to_string())?;

        let from_account = accounts
            .get(self.from_account_idx)
            .ok_or(t(locale, "err.no_source_account"))?;
        let to_account = accounts
            .get(self.to_account_idx)
            .ok_or(t(locale, "err.no_dest_account"))?;

        if from_account.id == to_account.id {
            return Err(t(locale, "err.same_account").into());
        }

        let amount = parse_positive_amount(&self.amount.value, locale)?;

        let description = self.description.value.trim().to_string();
        if description.is_empty() {
            return Err(t(locale, "err.description_required").into());
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
    if app.xfer.form.is_some() {
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
        t(app.locale, "header.from"),
        t(app.locale, "header.to"),
        t(app.locale, "header.amount"),
        t(app.locale, "header.description"),
    ])
    .style(Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .xfer.items
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
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(if app.xfer.count > 0 {
                let start = app.xfer.offset + 1;
                let end = app.xfer.offset + app.xfer.items.len() as u64;
                tf_paginated(app.locale, t(app.locale, "title.transfers"), start, end, app.xfer.count)
            } else {
                format!("{} (0)", t(app.locale, "title.transfers"))
            }),
    )
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.xfer.table_state);

    let detail_content = match app
        .xfer.table_state
        .selected()
        .and_then(|i| app.xfer.items.get(i))
    {
        Some(xf) => {
            vec![
                Line::from(format!(
                    " {} | {} -> {}",
                    xf.date.format("%d-%m-%Y"),
                    app.account_name(xf.from_account_id),
                    app.account_name(xf.to_account_id),
                )),
                Line::from(format!(" {} | {}", format_brl(xf.amount), xf.description,)),
                Line::from(format!(
                    " {}: {}",
                    t(app.locale, "detail.created"),
                    xf.created_at.format("%d-%m-%Y %H:%M")
                )),
                Line::from(Span::styled(
                    format!(" {}", t(app.locale, "hint.xfer")),
                    Style::new().fg(Color::DarkGray),
                )),
            ]
        }
        None => vec![Line::from(format!(" {}", t(app.locale, "misc.no_sel.transfer")))],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title(t(app.locale, "title.transfer_details"));
    let detail = Paragraph::new(detail_content).block(detail_block);
    frame.render_widget(detail, detail_area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.xfer.form.as_ref().unwrap();

    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in TransferField::ALL.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            TransferField::Date => form.date.render_line(active),
            TransferField::FromAccount => {
                let names: Vec<&str> = app.accounts.iter().map(|a| a.name.as_str()).collect();
                render_selector(t(app.locale, "form.from"), &names, form.from_account_idx, active, t(app.locale, "misc.no_accounts"), area.width)
            }
            TransferField::ToAccount => {
                let names: Vec<&str> = app.accounts.iter().map(|a| a.name.as_str()).collect();
                render_selector(t(app.locale, "form.to"), &names, form.to_account_idx, active, t(app.locale, "misc.no_accounts"), area.width)
            }
            TransferField::Amount => form.amount.render_line(active),
            TransferField::Description => form.description.render_line(active),
        };
        lines.push(line);
    }

    push_form_error(&mut lines, &form.error);

    let block = Block::default().borders(Borders::ALL).title(t(app.locale, "title.new_transfer"));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

// ── Key handling & form submission (impl App) ─────────────────────

impl App {
    pub(crate) async fn handle_transfers_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_selection(&mut self.xfer.table_state, self.xfer.items.len(), -1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_selection(&mut self.xfer.table_state, self.xfer.items.len(), 1);
            }
            KeyCode::Char('n') => {
                if self.accounts.len() < 2 {
                    self.status_message = Some(StatusMessage::error(
                        t(self.locale, "msg.need_two_accounts"),
                    ));
                } else {
                    self.xfer.form = Some(TransferForm::new_create(self.locale));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(t) = self
                    .xfer.table_state
                    .selected()
                    .and_then(|i| self.xfer.items.get(i))
                {
                    let t_id = t.id;
                    let t_desc = t.description.clone();
                    self.confirm_action = Some(ConfirmAction::DeleteTransfer(t_id));
                    self.confirm_popup = Some(ConfirmPopup::new(
                        crate::ui::i18n::tf_delete_transfer(self.locale, &t_desc)
                    ));
                }
            }
            KeyCode::Char('x') => {
                match crate::db::transfers::list_all_transfers(&self.pool).await {
                    Ok(all_transfers) => {
                        let acct_names = &self.account_names;
                        match crate::export::export_transfers(&all_transfers, |id| {
                            acct_names.get(&id).cloned().unwrap_or_else(|| "?".into())
                        }) {
                            Ok(path) => {
                                self.status_message = Some(StatusMessage::info(
                                    crate::ui::i18n::tf_exported(self.locale, all_transfers.len(), t(self.locale, "export.transfers"), &path)
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
                if self.xfer.offset > 0 {
                    self.xfer.offset = self.xfer.offset.saturating_sub(PAGE_SIZE);
                    self.load_transfers().await?;
                    self.xfer.table_state.select(Some(0));
                }
            }
            KeyCode::PageDown => {
                let next = self.xfer.offset + PAGE_SIZE;
                if next < self.xfer.count {
                    self.xfer.offset = next;
                    self.load_transfers().await?;
                    self.xfer.table_state.select(Some(0));
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_transfer_form_key(&mut self, code: KeyCode) {
        let form = self.xfer.form.as_mut().unwrap();
        match code {
            KeyCode::Tab | KeyCode::Down => {
                if form.active_field < TransferField::ALL.len() - 1 {
                    form.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if form.active_field > 0 {
                    form.active_field -= 1;
                }
            }
            _ => match form.active_field_id() {
                TransferField::Date => form.date.handle_key(code),
                TransferField::FromAccount => {
                    if crate::ui::app::is_toggle_key(code) {
                        cycle_index(&mut form.from_account_idx, self.accounts.len(), code);
                    }
                }
                TransferField::ToAccount => {
                    if crate::ui::app::is_toggle_key(code) {
                        cycle_index(&mut form.to_account_idx, self.accounts.len(), code);
                    }
                }
                TransferField::Amount => form.amount.handle_key(code),
                TransferField::Description => form.description.handle_key(code),
            },
        }
    }

    pub(crate) async fn submit_transfer_form(&mut self) -> anyhow::Result<()> {
        use crate::db::transfers;

        let form = self.xfer.form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts, self.locale) {
            Ok(v) => v,
            Err(e) => {
                self.xfer.form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        transfers::create_transfer(
            &self.pool,
            validated.from_account_id,
            validated.to_account_id,
            validated.amount,
            &validated.description,
            validated.date,
        )
        .await?;
        info!(
            desc = %validated.description,
            amount = %validated.amount,
            "transfer created"
        );

        self.xfer.form = None;
        self.input_mode = InputMode::Normal;
        self.load_transfers().await?;
        self.refresh_balances().await?;
        Ok(())
    }

    pub(crate) async fn execute_delete_transfer(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::transfers::delete_transfer(&self.pool, id).await?;
        info!(id, "transfer deleted");
        self.load_transfers().await?;
        self.refresh_balances().await?;
        clamp_selection(&mut self.xfer.table_state, self.xfer.items.len());
        Ok(())
    }
}
