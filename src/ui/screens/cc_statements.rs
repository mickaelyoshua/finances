use chrono::{Datelike, Local, NaiveDate};
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{
    db::{
        self, clamped_day, latest_closing_date, next_month, prev_month, statement_due_date,
        statement_period,
    },
    models::Account,
    ui::{
        App,
        app::{InputMode, StatusMessage, clamp_selection, cycle_index, move_table_selection},
        components::format::format_brl,
        screens::cc_payments::CcPaymentForm,
    },
};

// ── Data structures ───────────────────────────────────────────────

/// A computed credit card statement for one billing cycle.
#[derive(Debug, Clone)]
pub struct CreditCardStatement {
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub due_date: NaiveDate,
    pub total_charges: Decimal,
    pub total_credits: Decimal,
    pub statement_total: Decimal,
    pub paid_amount: Decimal,
    pub is_current: bool,
}

impl CreditCardStatement {
    pub fn balance_due(&self) -> Decimal {
        (self.statement_total - self.paid_amount).max(Decimal::ZERO)
    }

    pub fn status_label(&self) -> &'static str {
        if self.is_current {
            "Open"
        } else if self.balance_due() == Decimal::ZERO {
            "Paid"
        } else {
            "Due"
        }
    }

    /// Label like "03/2026" derived from the closing date.
    pub fn label(&self) -> String {
        self.period_end.format("%m/%Y").to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementsView {
    List,
    Detail,
}

// ── Statement builder ─────────────────────────────────────────────

/// Build the last `months` credit card statements for an account,
/// plus the current (open) statement.
pub async fn build_statements(
    pool: &PgPool,
    account: &Account,
    months: usize,
) -> anyhow::Result<Vec<CreditCardStatement>> {
    let billing_day = account.billing_day.unwrap_or(1) as u32;
    let due_day = account.due_day.unwrap_or(1) as u32;
    let today = Local::now().date_naive();

    // closing_dates is ordered most-recent-first (index 0 = latest close).
    // Payment attribution relies on this ordering: idx-1 is the next-newer close.
    let mut closing_dates: Vec<NaiveDate> = Vec::with_capacity(months + 1);
    let latest_close = latest_closing_date(today, billing_day);

    closing_dates.push(latest_close);
    for _ in 1..months {
        let prev = closing_dates.last().unwrap();
        let (y, m) = prev_month(prev.year(), prev.month());
        closing_dates.push(clamped_day(y, m, billing_day));
    }

    // Earliest start date (one more month back for the oldest statement's period)
    let oldest_close = closing_dates.last().unwrap();
    let (start_period, _) = statement_period(*oldest_close, billing_day);

    // Fetch all credit transactions and CC payments in the full range
    let (transactions, payments) = tokio::try_join!(
        db::transactions::list_credit_by_account(pool, account.id, start_period, today),
        db::credit_card_payments::list_payments_in_range(pool, account.id, start_period, today),
    )?;

    let mut statements: Vec<CreditCardStatement> = Vec::with_capacity(months + 1);

    // Build closed statements (most recent first)
    for close_date in &closing_dates {
        let (period_start, period_end) = statement_period(*close_date, billing_day);
        let due = statement_due_date(close_date.year(), close_date.month(), billing_day, due_day);

        let mut charges = Decimal::ZERO;
        let mut credits = Decimal::ZERO;
        for t in &transactions {
            if t.date >= period_start && t.date <= period_end {
                if t.parsed_type() == crate::models::TransactionType::Expense {
                    charges += t.amount;
                } else {
                    credits += t.amount;
                }
            }
        }

        // Payment attribution: payments made between this close+1 and the next close
        // are credited to this statement. Limitation: a late payment (made after the
        // next close) will be attributed to the wrong statement since there is no
        // explicit statement reference on credit_card_payments.
        let pay_start = close_date.succ_opt().unwrap();
        let pay_end = if close_date == &latest_close {
            today
        } else {
            // Find the next closing date
            let idx = closing_dates.iter().position(|d| d == close_date).unwrap();
            if idx > 0 {
                closing_dates[idx - 1]
            } else {
                today
            }
        };

        let paid: Decimal = payments
            .iter()
            .filter(|p| p.date >= pay_start && p.date <= pay_end)
            .map(|p| p.amount)
            .sum();

        let total = charges - credits;
        statements.push(CreditCardStatement {
            period_start,
            period_end,
            due_date: due,
            total_charges: charges,
            total_credits: credits,
            statement_total: total,
            paid_amount: paid,
            is_current: false,
        });
    }

    // Build current (open) statement
    let current_start = latest_close.succ_opt().unwrap();
    if current_start <= today {
        let (next_y, next_m) = next_month(latest_close.year(), latest_close.month());
        let next_close = clamped_day(next_y, next_m, billing_day);
        let due = statement_due_date(next_y, next_m, billing_day, due_day);

        let mut charges = Decimal::ZERO;
        let mut credits = Decimal::ZERO;
        for t in &transactions {
            if t.date >= current_start && t.date <= today {
                if t.parsed_type() == crate::models::TransactionType::Expense {
                    charges += t.amount;
                } else {
                    credits += t.amount;
                }
            }
        }

        let total = charges - credits;
        statements.insert(
            0,
            CreditCardStatement {
                period_start: current_start,
                period_end: next_close,
                due_date: due,
                total_charges: charges,
                total_credits: credits,
                statement_total: total,
                paid_amount: Decimal::ZERO,
                is_current: true,
            },
        );
    }

    Ok(statements)
}

// ── Render ────────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    match app.cc_statement_view {
        StatementsView::List => render_list(frame, area, app),
        StatementsView::Detail => render_detail(frame, area, app),
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let [selector_area, table_area, detail_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(5),
    ])
    .areas(area);

    // Account selector
    let cc_accounts: Vec<&str> = app
        .accounts
        .iter()
        .filter(|a| a.has_credit_card)
        .map(|a| a.name.as_str())
        .collect();

    let selector_text = if cc_accounts.is_empty() {
        Line::from(Span::styled(
            " No credit card accounts",
            Style::new().fg(Color::DarkGray),
        ))
    } else {
        let name = cc_accounts
            .get(app.cc_statement_account_idx)
            .unwrap_or(&"?");
        Line::from(vec![
            Span::styled(" Account: ", Style::new().fg(Color::DarkGray)),
            Span::styled(
                format!("◀ {} ▶", name),
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "  ({}/{})",
                    app.cc_statement_account_idx + 1,
                    cc_accounts.len()
                ),
                Style::new().fg(Color::DarkGray),
            ),
        ])
    };
    frame.render_widget(Paragraph::new(selector_text), selector_area);

    // Statement table
    let header = Row::new([
        "Period", "Due", "Charges", "Credits", "Total", "Paid", "Balance", "Status",
    ])
    .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .cc_statements
        .iter()
        .map(|s| {
            let status_style = match s.status_label() {
                "Open" => Style::new().fg(Color::Cyan),
                "Paid" => Style::new().fg(Color::Green),
                _ => Style::new().fg(Color::Red),
            };
            Row::new([
                format!(
                    "{} - {}",
                    s.period_start.format("%d/%m"),
                    s.period_end.format("%d/%m/%y")
                ),
                s.due_date.format("%d/%m").to_string(),
                format_brl(s.total_charges),
                format_brl(s.total_credits),
                format_brl(s.statement_total),
                format_brl(s.paid_amount),
                format_brl(s.balance_due()),
                s.status_label().to_string(),
            ])
            .style(status_style)
        })
        .collect();

    let title = if cc_accounts.is_empty() {
        "CC Statements".to_string()
    } else {
        let name = cc_accounts
            .get(app.cc_statement_account_idx)
            .unwrap_or(&"?");
        format!("CC Statements — {}", name)
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(18), // Period
            Constraint::Length(7),  // Due
            Constraint::Length(14), // Charges
            Constraint::Length(14), // Credits
            Constraint::Length(14), // Total
            Constraint::Length(14), // Paid
            Constraint::Length(14), // Balance
            Constraint::Length(6),  // Status
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title))
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.cc_statement_table_state);

    // Detail pane
    let detail_content = match app
        .cc_statement_table_state
        .selected()
        .and_then(|i| app.cc_statements.get(i))
    {
        Some(s) => {
            vec![
                Line::from(format!(
                    " {} | {} - {} | Due: {}",
                    s.label(),
                    s.period_start.format("%d/%m/%Y"),
                    s.period_end.format("%d/%m/%Y"),
                    s.due_date.format("%d/%m/%Y"),
                )),
                Line::from(format!(
                    " Charges: {}  Credits: {}  Total: {}  Paid: {}  Balance: {}",
                    format_brl(s.total_charges),
                    format_brl(s.total_credits),
                    format_brl(s.statement_total),
                    format_brl(s.paid_amount),
                    format_brl(s.balance_due()),
                )),
                Line::from(Span::styled(
                    " [Enter] View transactions  [p] Pay  [h/l] Switch account",
                    Style::new().fg(Color::DarkGray),
                )),
            ]
        }
        None => vec![Line::from(" No statements available.")],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("Statement Details");
    frame.render_widget(
        Paragraph::new(detail_content).block(detail_block),
        detail_area,
    );
}

fn render_detail(frame: &mut Frame, area: Rect, app: &mut App) {
    let [header_area, table_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).areas(area);

    // Header with statement info
    let stmt_info = if let Some(idx) = app.cc_statement_table_state.selected() {
        if let Some(s) = app.cc_statements.get(idx) {
            let cc_accounts: Vec<&str> = app
                .accounts
                .iter()
                .filter(|a| a.has_credit_card)
                .map(|a| a.name.as_str())
                .collect();
            let account_name = cc_accounts
                .get(app.cc_statement_account_idx)
                .unwrap_or(&"?");
            vec![
                Line::from(format!(
                    " {} — {} | {} - {} | Total: {} | Status: {}",
                    account_name,
                    s.label(),
                    s.period_start.format("%d/%m/%Y"),
                    s.period_end.format("%d/%m/%Y"),
                    format_brl(s.statement_total),
                    s.status_label(),
                )),
                Line::from(Span::styled(
                    " [Esc] Back to statements",
                    Style::new().fg(Color::DarkGray),
                )),
            ]
        } else {
            vec![Line::from(" No statement selected.")]
        }
    } else {
        vec![Line::from(" No statement selected.")]
    };

    let header_block = Block::default().borders(Borders::ALL).title("Statement");
    frame.render_widget(Paragraph::new(stmt_info).block(header_block), header_area);

    // Transactions table
    let table_header = Row::new(["Date", "Description", "Category", "Type", "Amount"])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .cc_statement_detail_txns
        .iter()
        .map(|t| {
            let type_style = if t.parsed_type() == crate::models::TransactionType::Expense {
                Style::new().fg(Color::Red)
            } else {
                Style::new().fg(Color::Green)
            };
            Row::new([
                t.date.format("%d/%m/%Y").to_string(),
                t.description.clone(),
                app.category_name(t.category_id).to_string(),
                t.parsed_type().to_string(),
                format_brl(t.amount),
            ])
            .style(type_style)
        })
        .collect();

    let count = app.cc_statement_detail_txns.len();
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Min(20),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Length(14),
        ],
    )
    .header(table_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Transactions ({count})")),
    )
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.cc_statement_detail_table_state);
}

// ── Key handling & loaders (impl App) ─────────────────────────────

impl App {
    pub(crate) async fn handle_cc_statements_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match self.cc_statement_view {
            StatementsView::List => self.handle_cc_statements_list_key(code).await,
            StatementsView::Detail => self.handle_cc_statements_detail_key(code).await,
        }
    }

    async fn handle_cc_statements_list_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_selection(
                    &mut self.cc_statement_table_state,
                    self.cc_statements.len(),
                    -1,
                );
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_selection(
                    &mut self.cc_statement_table_state,
                    self.cc_statements.len(),
                    1,
                );
            }
            KeyCode::Char('h') | KeyCode::Char('l') => {
                let cc_count = self.accounts.iter().filter(|a| a.has_credit_card).count();
                if cc_count > 1 {
                    cycle_index(&mut self.cc_statement_account_idx, cc_count, code);
                    self.load_cc_statements().await?;
                }
            }
            KeyCode::Enter => {
                if let Some(idx) = self.cc_statement_table_state.selected()
                    && idx < self.cc_statements.len()
                {
                    self.load_cc_statement_detail(idx).await?;
                    self.cc_statement_view = StatementsView::Detail;
                }
            }
            KeyCode::Char('p') => {
                if let Some(stmt) = self
                    .cc_statement_table_state
                    .selected()
                    .and_then(|i| self.cc_statements.get(i))
                {
                    if stmt.is_current {
                        self.status_message =
                            Some(StatusMessage::error("Cannot pay an open statement"));
                    } else if stmt.balance_due() == Decimal::ZERO {
                        self.status_message =
                            Some(StatusMessage::info("Statement already fully paid"));
                    } else {
                        let balance = stmt.balance_due();
                        let label = stmt.label();
                        let mut form = CcPaymentForm::new_create();
                        form.account_idx = self.cc_statement_account_idx;
                        form.amount.value = balance.to_string();
                        form.amount.cursor = form.amount.value.len();
                        form.description.value = format!("Fatura {}", label);
                        form.description.cursor = form.description.value.len();
                        self.cc_payment_form = Some(form);
                        self.input_mode = InputMode::Editing;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_cc_statements_detail_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_selection(
                    &mut self.cc_statement_detail_table_state,
                    self.cc_statement_detail_txns.len(),
                    -1,
                );
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_selection(
                    &mut self.cc_statement_detail_table_state,
                    self.cc_statement_detail_txns.len(),
                    1,
                );
            }
            KeyCode::Esc => {
                self.cc_statement_view = StatementsView::List;
                self.cc_statement_detail_txns.clear();
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn load_cc_statements(&mut self) -> anyhow::Result<()> {
        let cc_accounts: Vec<&Account> =
            self.accounts.iter().filter(|a| a.has_credit_card).collect();
        if let Some(account) = cc_accounts.get(self.cc_statement_account_idx) {
            self.cc_statements = build_statements(&self.pool, account, 12).await?;
        } else {
            self.cc_statements.clear();
        }
        clamp_selection(&mut self.cc_statement_table_state, self.cc_statements.len());
        self.cc_statement_view = StatementsView::List;
        self.cc_statement_detail_txns.clear();
        Ok(())
    }

    async fn load_cc_statement_detail(&mut self, stmt_idx: usize) -> anyhow::Result<()> {
        if let Some(stmt) = self.cc_statements.get(stmt_idx) {
            let cc_accounts: Vec<&Account> =
                self.accounts.iter().filter(|a| a.has_credit_card).collect();
            if let Some(account) = cc_accounts.get(self.cc_statement_account_idx) {
                let end = if stmt.is_current {
                    Local::now().date_naive()
                } else {
                    stmt.period_end
                };
                self.cc_statement_detail_txns = db::transactions::list_credit_by_account(
                    &self.pool,
                    account.id,
                    stmt.period_start,
                    end,
                )
                .await?;
                self.cc_statement_detail_table_state.select(
                    if self.cc_statement_detail_txns.is_empty() {
                        None
                    } else {
                        Some(0)
                    },
                );
            }
        }
        Ok(())
    }
}
