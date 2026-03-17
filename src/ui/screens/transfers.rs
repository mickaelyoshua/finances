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
            ConfirmAction, InputMode, StatusMessage, clamp_selection, cycle_index,
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
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);

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
                Line::from(Span::styled(
                    " [n] New  [d] Delete  [x] Export",
                    Style::new().fg(Color::DarkGray),
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

// ── Key handling & form submission (impl App) ─────────────────────

impl App {
    pub(crate) async fn handle_transfers_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up => {
                move_table_selection(&mut self.transfer_table_state, self.transfers.len(), -1);
            }
            KeyCode::Down => {
                move_table_selection(&mut self.transfer_table_state, self.transfers.len(), 1);
            }
            KeyCode::Char('n') => {
                if self.accounts.len() < 2 {
                    self.status_message = Some(StatusMessage::error(
                        "Need at least 2 accounts for a transfer",
                    ));
                } else {
                    self.transfer_form = Some(TransferForm::new_create());
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(t) = self
                    .transfer_table_state
                    .selected()
                    .and_then(|i| self.transfers.get(i))
                {
                    let t_id = t.id;
                    let t_desc = t.description.clone();
                    self.confirm_action = Some(ConfirmAction::DeleteTransfer(t_id));
                    self.confirm_popup = Some(ConfirmPopup::new(format!(
                        "Delete transfer \"{}\"?",
                        t_desc
                    )));
                }
            }
            KeyCode::Char('x') => {
                let acct_names = &self.account_names;
                match crate::export::export_transfers(
                    &self.transfers,
                    |id| acct_names.get(&id).cloned().unwrap_or_else(|| "?".into()),
                ) {
                    Ok(path) => {
                        self.status_message = Some(StatusMessage::info(format!(
                            "Exported {} transfers to {}",
                            self.transfers.len(),
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

    pub(crate) fn handle_transfer_form_key(&mut self, code: KeyCode) {
        let form = self.transfer_form.as_mut().unwrap();
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

        let form = self.transfer_form.as_ref().unwrap();
        let validated = match form.validate(&self.accounts) {
            Ok(v) => v,
            Err(e) => {
                self.transfer_form.as_mut().unwrap().error = Some(e);
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

        self.transfer_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    pub(crate) async fn execute_delete_transfer(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::transfers::delete_transfer(&self.pool, id).await?;
        info!(id, "transfer deleted");
        self.load_data().await?;
        clamp_selection(&mut self.transfer_table_state, self.transfers.len());
        Ok(())
    }
}
