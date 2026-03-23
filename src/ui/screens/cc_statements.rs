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
        app::{InputMode, Screen, StatusMessage, clamp_selection, cycle_index, move_table_selection},
        components::format::format_brl,
        screens::transactions::TransactionForm,
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
    pub is_upcoming: bool,
}

impl CreditCardStatement {
    pub fn balance_due(&self) -> Decimal {
        (self.statement_total - self.paid_amount).max(Decimal::ZERO)
    }

    pub fn status_label(&self) -> &'static str {
        if self.is_upcoming {
            "Upcoming"
        } else if self.is_current {
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

/// Build credit card statements for an account: past closed, current open,
/// and future projected (based on existing future transactions like installments).
pub async fn build_statements(
    pool: &PgPool,
    account: &Account,
    months: usize,
) -> anyhow::Result<(Vec<CreditCardStatement>, usize)> {
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

    // Build future closing dates based on the latest credit transaction date
    let max_txn_date = db::transactions::max_credit_date(pool, account.id).await?;
    let mut future_closing_dates: Vec<NaiveDate> = Vec::new();
    if let Some(max_date) = max_txn_date {
        let mut cursor = latest_close;
        loop {
            let (ny, nm) = next_month(cursor.year(), cursor.month());
            cursor = clamped_day(ny, nm, billing_day);
            if cursor <= latest_close {
                break; // safety guard
            }
            future_closing_dates.push(cursor);
            // Once cursor passes max_date, this closing date covers the last period
            if cursor >= max_date {
                break;
            }
        }
    }

    // Data range: from oldest past period to the latest future closing date
    let oldest_close = closing_dates.last().unwrap();
    let (start_period, _) = statement_period(*oldest_close, billing_day);
    let end_date = future_closing_dates.last().copied().unwrap_or(today);

    // Fetch all credit transactions and CC payments in the full range
    let (transactions, payments) = tokio::try_join!(
        db::transactions::list_credit_by_account(pool, account.id, start_period, end_date),
        db::credit_card_payments::list_payments_in_range(pool, account.id, start_period, end_date),
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

        // Payment attribution: card bills are paid after the statement closes,
        // so payments between this close+1 and the next-newer close date
        // are credited to this statement (not the one they fall within).
        let pay_start = close_date.succ_opt().unwrap();
        let pay_end = if close_date == &latest_close {
            today
        } else {
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
            is_upcoming: false,
        });
    }

    // Build current (open) statement
    let current_start = latest_close.succ_opt().unwrap();
    let (next_y, next_m) = next_month(latest_close.year(), latest_close.month());
    let current_close = clamped_day(next_y, next_m, billing_day);
    let current_due = statement_due_date(next_y, next_m, billing_day, due_day);

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
            period_end: current_close,
            due_date: current_due,
            total_charges: charges,
            total_credits: credits,
            statement_total: total,
            paid_amount: Decimal::ZERO,
            is_current: true,
            is_upcoming: false,
        },
    );

    // The current/open statement is at index 0 right now
    let mut current_idx: usize = 0;

    // Build future (upcoming) statements — nearest future first
    for close_date in &future_closing_dates {
        // Skip the close date that matches the current open statement
        if *close_date == current_close {
            continue;
        }

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

        let total = charges - credits;
        // Skip future statements with no charges
        if total == Decimal::ZERO {
            continue;
        }

        statements.insert(
            0,
            CreditCardStatement {
                period_start,
                period_end,
                due_date: due,
                total_charges: charges,
                total_credits: credits,
                statement_total: total,
                paid_amount: Decimal::ZERO,
                is_current: false,
                is_upcoming: true,
            },
        );
        current_idx += 1; // current statement shifts down
    }

    Ok((statements, current_idx))
}

/// Build aggregated statements across all CC accounts, grouped by closing month.
async fn build_all_accounts_statements(
    pool: &PgPool,
    cc_accounts: &[&Account],
) -> anyhow::Result<Vec<CreditCardStatement>> {
    use std::collections::BTreeMap;

    if cc_accounts.is_empty() {
        return Ok(Vec::new());
    }

    // Build statements for each account and collect them
    let mut all_stmts: Vec<CreditCardStatement> = Vec::new();
    for account in cc_accounts {
        let (stmts, _) = build_statements(pool, account, 12).await?;
        all_stmts.extend(stmts);
    }

    // Group by closing month (year, month from period_end)
    // Use BTreeMap for sorted order (newest first when reversed)
    let mut by_month: BTreeMap<(i32, u32), Vec<&CreditCardStatement>> = BTreeMap::new();
    for stmt in &all_stmts {
        let key = (stmt.period_end.year(), stmt.period_end.month());
        by_month.entry(key).or_default().push(stmt);
    }

    // Aggregate each month group
    let mut aggregated: Vec<CreditCardStatement> = by_month
        .into_iter()
        .map(|((_year, _month), stmts)| {
            let total_charges: Decimal = stmts.iter().map(|s| s.total_charges).sum();
            let total_credits: Decimal = stmts.iter().map(|s| s.total_credits).sum();
            let statement_total: Decimal = stmts.iter().map(|s| s.statement_total).sum();
            let paid_amount: Decimal = stmts.iter().map(|s| s.paid_amount).sum();
            let is_current = stmts.iter().any(|s| s.is_current);
            let is_upcoming = stmts.iter().all(|s| s.is_upcoming);

            // Use the widest date range and latest due date
            let period_start = stmts.iter().map(|s| s.period_start).min().unwrap();
            let period_end = stmts.iter().map(|s| s.period_end).max().unwrap();
            let due_date = stmts.iter().map(|s| s.due_date).max().unwrap();

            CreditCardStatement {
                period_start,
                period_end,
                due_date,
                total_charges,
                total_credits,
                statement_total,
                paid_amount,
                is_current,
                is_upcoming,
            }
        })
        .collect();

    // Sort: newest (biggest period_end) first, matching individual account view order
    aggregated.sort_by(|a, b| b.period_end.cmp(&a.period_end));

    Ok(aggregated)
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

    // Account selector (index 0 = "All Accounts", 1+ = individual accounts)
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
        let display_name = if app.cc_statement_account_idx == 0 {
            "All Accounts"
        } else {
            cc_accounts
                .get(app.cc_statement_account_idx - 1)
                .unwrap_or(&"?")
        };
        let total_options = cc_accounts.len() + 1; // +1 for "All"
        Line::from(vec![
            Span::styled(" Account: ", Style::new().fg(Color::DarkGray)),
            Span::styled(
                format!("◀ {} ▶", display_name),
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "  ({}/{})",
                    app.cc_statement_account_idx + 1,
                    total_options
                ),
                Style::new().fg(Color::DarkGray),
            ),
        ])
    };
    frame.render_widget(Paragraph::new(selector_text), selector_area);

    // Statement table
    let header = Row::new(["Period", "Total", "Balance", "Status"])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .cc_statements
        .iter()
        .map(|s| {
            let status_style = match s.status_label() {
                "Upcoming" => Style::new().fg(Color::Blue),
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
                format_brl(s.statement_total),
                format_brl(s.balance_due()),
                s.status_label().to_string(),
            ])
            .style(status_style)
        })
        .collect();

    let title = if cc_accounts.is_empty() {
        "CC Statements".to_string()
    } else if app.cc_statement_account_idx == 0 {
        "CC Statements — All Accounts".to_string()
    } else {
        let name = cc_accounts
            .get(app.cc_statement_account_idx - 1)
            .unwrap_or(&"?");
        format!("CC Statements — {}", name)
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(18), // Period
            Constraint::Length(14), // Total
            Constraint::Length(14), // Balance
            Constraint::Length(10), // Status
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
                    if app.cc_statement_account_idx == 0 {
                        " [h/l] Switch account"
                    } else {
                        " [Enter] View transactions  [p] Pay  [u] Unpay  [h/l] Switch account"
                    },
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
    let [header_area, table_area, detail_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(5), Constraint::Length(3)])
            .areas(area);

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
            vec![Line::from(format!(
                " {} — {} | {} - {} | Total: {} | Status: {}",
                account_name,
                s.label(),
                s.period_start.format("%d/%m/%Y"),
                s.period_end.format("%d/%m/%Y"),
                format_brl(s.statement_total),
                s.status_label(),
            ))]
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

    // Detail pane with key guide
    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("Details");
    let detail_content = vec![Line::from(Span::styled(
        " [Esc] Back  [Enter] Go to transaction / installment  [j/k] Navigate",
        Style::new().fg(Color::DarkGray),
    ))];
    frame.render_widget(
        Paragraph::new(detail_content).block(detail_block),
        detail_area,
    );
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
                // +1 for "All Accounts" at index 0
                cycle_index(&mut self.cc_statement_account_idx, cc_count + 1, code);
                self.load_cc_statements().await?;
            }
            KeyCode::Enter => {
                if self.cc_statement_account_idx == 0 {
                    self.status_message =
                        Some(StatusMessage::info("Select a specific account to view transactions"));
                } else if let Some(idx) = self.cc_statement_table_state.selected()
                    && idx < self.cc_statements.len()
                {
                    self.load_cc_statement_detail(idx).await?;
                    self.cc_statement_view = StatementsView::Detail;
                }
            }
            KeyCode::Char('p') => {
                if self.cc_statement_account_idx == 0 {
                    self.status_message =
                        Some(StatusMessage::info("Select a specific account to pay a statement"));
                } else if let Some(stmt) = self
                    .cc_statement_table_state
                    .selected()
                    .and_then(|i| self.cc_statements.get(i))
                {
                    if stmt.is_upcoming {
                        self.status_message =
                            Some(StatusMessage::error("Cannot pay an upcoming statement"));
                    } else if stmt.is_current {
                        self.status_message =
                            Some(StatusMessage::error("Cannot pay an open statement"));
                    } else if stmt.balance_due() == Decimal::ZERO {
                        self.status_message =
                            Some(StatusMessage::info("Statement already fully paid"));
                    } else {
                        let cc_accounts: Vec<&crate::models::Account> =
                            self.accounts.iter().filter(|a| a.has_credit_card).collect();
                        if let Some(account) = cc_accounts.get(self.cc_statement_account_idx - 1) {
                            let balance = stmt.balance_due();
                            let label = stmt.label();
                            let pay_date = stmt.period_end.succ_opt().unwrap();
                            self.confirm_action = Some(crate::ui::app::ConfirmAction::PayCreditCardStatement {
                                account_id: account.id,
                                amount: balance,
                                date: pay_date,
                                description: format!("Fatura {}", label),
                            });
                            self.confirm_popup = Some(crate::ui::components::popup::ConfirmPopup::new(
                                format!("Pay statement {}? ({})", label, crate::ui::components::format::format_brl(balance)),
                            ));
                        }
                    }
                }
            }
            KeyCode::Char('u') => {
                if self.cc_statement_account_idx == 0 {
                    self.status_message =
                        Some(StatusMessage::info("Select a specific account to unpay a statement"));
                } else if let Some((idx, stmt)) = self
                    .cc_statement_table_state
                    .selected()
                    .and_then(|i| self.cc_statements.get(i).map(|s| (i, s)))
                {
                    if stmt.is_upcoming || stmt.is_current {
                        self.status_message =
                            Some(StatusMessage::error("Only closed statements can be unpaid"));
                    } else if stmt.paid_amount == Decimal::ZERO {
                        self.status_message =
                            Some(StatusMessage::info("Statement has no payments"));
                    } else {
                        let cc_accounts: Vec<&crate::models::Account> =
                            self.accounts.iter().filter(|a| a.has_credit_card).collect();
                        if let Some(account) = cc_accounts.get(self.cc_statement_account_idx - 1) {
                            let pay_start = stmt.period_end.succ_opt().unwrap();
                            let pay_end = if idx > 0 {
                                let prev = &self.cc_statements[idx - 1];
                                if prev.is_current || prev.is_upcoming {
                                    Local::now().date_naive()
                                } else {
                                    prev.period_end
                                }
                            } else {
                                Local::now().date_naive()
                            };
                            let label = stmt.label();
                            let paid = stmt.paid_amount;
                            self.confirm_action = Some(crate::ui::app::ConfirmAction::UnpayCreditCardStatement {
                                account_id: account.id,
                                pay_start,
                                pay_end,
                            });
                            self.confirm_popup = Some(crate::ui::components::popup::ConfirmPopup::new(
                                format!("Remove all payments for {}? ({})", label, crate::ui::components::format::format_brl(paid)),
                            ));
                        }
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
            KeyCode::Enter => {
                if let Some(txn) = self
                    .cc_statement_detail_table_state
                    .selected()
                    .and_then(|i| self.cc_statement_detail_txns.get(i))
                {
                    if let Some(ip_id) = txn.installment_purchase_id {
                        // Navigate to Installments screen and select the parent purchase
                        self.screen = Screen::Installments;
                        if let Some(pos) = self.installments.iter().position(|ip| ip.id == ip_id) {
                            self.installment_table_state.select(Some(pos));
                        }
                    } else {
                        // Regular transaction — open edit form on Transactions screen
                        let txn = txn.clone();
                        self.transaction_form = Some(TransactionForm::new_edit(
                            &txn,
                            &self.accounts,
                            &self.categories,
                        ));
                        self.screen = Screen::Transactions;
                        self.input_mode = InputMode::Editing;
                        self.load_transactions().await?;
                    }
                    self.cc_statement_view = StatementsView::List;
                    self.cc_statement_detail_txns.clear();
                }
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

        if self.cc_statement_account_idx == 0 {
            // "All Accounts" — aggregate across all CC accounts by closing month
            self.cc_statements = build_all_accounts_statements(&self.pool, &cc_accounts).await?;
            // Select the current/open statement
            let current_idx = self
                .cc_statements
                .iter()
                .position(|s| s.is_current)
                .unwrap_or(0);
            if !self.cc_statements.is_empty() {
                self.cc_statement_table_state
                    .select(Some(current_idx.min(self.cc_statements.len() - 1)));
            }
        } else if let Some(account) = cc_accounts.get(self.cc_statement_account_idx - 1) {
            let (stmts, current_idx) = build_statements(&self.pool, account, 12).await?;
            self.cc_statements = stmts;
            // Default selection to the current/open statement
            if !self.cc_statements.is_empty() {
                self.cc_statement_table_state
                    .select(Some(current_idx.min(self.cc_statements.len() - 1)));
            }
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
            if let Some(account) = cc_accounts.get(self.cc_statement_account_idx - 1) {
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

    pub(crate) async fn execute_pay_cc_statement(
        &mut self,
        account_id: i32,
        amount: Decimal,
        date: NaiveDate,
        description: &str,
    ) -> anyhow::Result<()> {
        db::credit_card_payments::create_payment(&self.pool, account_id, amount, date, description)
            .await?;
        tracing::info!(account_id, %amount, description, "credit card statement paid");
        self.load_data().await?;
        self.load_cc_statements().await?;
        self.status_message = Some(StatusMessage::info("Statement paid"));
        Ok(())
    }

    pub(crate) async fn execute_unpay_cc_statement(
        &mut self,
        account_id: i32,
        pay_start: NaiveDate,
        pay_end: NaiveDate,
    ) -> anyhow::Result<()> {
        let deleted = db::credit_card_payments::delete_payments_in_range(
            &self.pool, account_id, pay_start, pay_end,
        )
        .await?;
        tracing::info!(account_id, %pay_start, %pay_end, deleted, "credit card statement unpaid");
        self.load_data().await?;
        self.load_cc_statements().await?;
        self.status_message = Some(StatusMessage::info(format!(
            "Removed {} payment{}",
            deleted,
            if deleted == 1 { "" } else { "s" }
        )));
        Ok(())
    }
}
