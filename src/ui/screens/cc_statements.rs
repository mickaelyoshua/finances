//! Credit card statement screen — lists billing cycles per account.
//!
//! [`build_statements`] reconstructs past, current (open), and future
//! (projected from installments) statements by walking backwards and
//! forwards from today's billing-day closing date. Payment attribution
//! matches CC payments that fall within each statement's date range.

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
        i18n::t,
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

    // ── Single-pass bucketing: assign each transaction/payment to its period ──
    // Build all period boundaries first, then walk transactions once.

    // Current period
    let current_start = latest_close.succ_opt().unwrap();
    let (next_y, next_m) = next_month(latest_close.year(), latest_close.month());
    let current_close = clamped_day(next_y, next_m, billing_day);
    let current_due = statement_due_date(next_y, next_m, billing_day, due_day);

    // Collect all periods: (period_start, period_end, kind)
    #[derive(Clone, Copy)]
    enum PeriodKind { Closed, Current, Upcoming }
    struct PeriodInfo {
        start: NaiveDate,
        end: NaiveDate,
        due: NaiveDate,
        kind: PeriodKind,
        charges: Decimal,
        credits: Decimal,
        paid: Decimal,
    }

    let mut periods: Vec<PeriodInfo> = Vec::with_capacity(months + future_closing_dates.len() + 1);

    // Closed periods (most recent first in closing_dates)
    for close_date in &closing_dates {
        let (ps, pe) = statement_period(*close_date, billing_day);
        let due = statement_due_date(close_date.year(), close_date.month(), billing_day, due_day);
        periods.push(PeriodInfo {
            start: ps, end: pe, due, kind: PeriodKind::Closed,
            charges: Decimal::ZERO, credits: Decimal::ZERO, paid: Decimal::ZERO,
        });
    }

    // Current period
    periods.push(PeriodInfo {
        start: current_start, end: current_close, due: current_due,
        kind: PeriodKind::Current,
        charges: Decimal::ZERO, credits: Decimal::ZERO, paid: Decimal::ZERO,
    });

    // Future periods
    for close_date in &future_closing_dates {
        if *close_date == current_close { continue; }
        let (ps, pe) = statement_period(*close_date, billing_day);
        let due = statement_due_date(close_date.year(), close_date.month(), billing_day, due_day);
        periods.push(PeriodInfo {
            start: ps, end: pe, due, kind: PeriodKind::Upcoming,
            charges: Decimal::ZERO, credits: Decimal::ZERO, paid: Decimal::ZERO,
        });
    }

    // Sort periods by start date for binary search
    periods.sort_by_key(|p| p.start);

    // Bucket transactions in a single pass
    for t in &transactions {
        // Binary search: find the period whose start <= t.date, then verify t.date <= end
        let idx = periods.partition_point(|p| p.start <= t.date);
        if idx > 0 {
            let p = &mut periods[idx - 1];
            if t.date <= p.end {
                if t.parsed_type() == crate::models::TransactionType::Expense {
                    p.charges += t.amount;
                } else {
                    p.credits += t.amount;
                }
            }
        }
    }

    // Bucket payments: payment attribution — payments after a close date are credited
    // to the statement that just closed. Build payment windows from closing dates.
    // For each closed period, payment window is (close_date+1 .. next_newer_close_date).
    // For the latest closed statement, window extends to today.
    {
        // Build a sorted list of closing dates (ascending) for payment window lookup
        let mut sorted_closes: Vec<NaiveDate> = closing_dates.clone();
        sorted_closes.sort();

        for payment in &payments {
            // Find which statement this payment is attributed to:
            // the largest closing date strictly less than payment.date
            let close_idx = sorted_closes.partition_point(|&c| c < payment.date);
            if close_idx > 0 {
                let attributed_close = sorted_closes[close_idx - 1];
                // Find the period with this closing date (end == close)
                if let Some(p) = periods.iter_mut().find(|p| p.end == attributed_close && matches!(p.kind, PeriodKind::Closed)) {
                    // Verify payment is within the attribution window
                    let pay_start = attributed_close.succ_opt().unwrap();
                    let pay_end = if close_idx < sorted_closes.len() {
                        sorted_closes[close_idx]
                    } else {
                        today
                    };
                    if payment.date >= pay_start && payment.date <= pay_end {
                        p.paid += payment.amount;
                    }
                }
            }
        }
    }

    // Build statement structs from periods
    let mut current_idx: usize = 0;
    for p in &periods {
        let total = p.charges - p.credits;
        match p.kind {
            PeriodKind::Upcoming => {
                if total == Decimal::ZERO { continue; }
                statements.insert(0, CreditCardStatement {
                    period_start: p.start, period_end: p.end, due_date: p.due,
                    total_charges: p.charges, total_credits: p.credits,
                    statement_total: total, paid_amount: Decimal::ZERO,
                    is_current: false, is_upcoming: true,
                });
                current_idx += 1;
            }
            PeriodKind::Current => {
                statements.insert(current_idx, CreditCardStatement {
                    period_start: p.start, period_end: p.end, due_date: p.due,
                    total_charges: p.charges, total_credits: p.credits,
                    statement_total: total, paid_amount: Decimal::ZERO,
                    is_current: true, is_upcoming: false,
                });
            }
            PeriodKind::Closed => {
                statements.push(CreditCardStatement {
                    period_start: p.start, period_end: p.end, due_date: p.due,
                    total_charges: p.charges, total_credits: p.credits,
                    statement_total: total, paid_amount: p.paid,
                    is_current: false, is_upcoming: false,
                });
            }
        }
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

    // Build statements for all accounts in parallel
    let mut handles = Vec::with_capacity(cc_accounts.len());
    for account in cc_accounts {
        let pool = pool.clone();
        let account = (*account).clone();
        handles.push(tokio::spawn(async move {
            build_statements(&pool, &account, 12).await
        }));
    }

    let mut all_stmts: Vec<CreditCardStatement> = Vec::new();
    for handle in handles {
        let (stmts, _) = handle.await??;
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
    match app.cc_stmt.view {
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
            format!(" {}", t(app.locale, "misc.no_cc_accts")),
            Style::new().fg(Color::DarkGray),
        ))
    } else {
        let display_name = if app.cc_stmt.account_idx == 0 {
            t(app.locale, "stmt.all_accounts")
        } else {
            cc_accounts
                .get(app.cc_stmt.account_idx - 1)
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
                    app.cc_stmt.account_idx + 1,
                    total_options
                ),
                Style::new().fg(Color::DarkGray),
            ),
        ])
    };
    frame.render_widget(Paragraph::new(selector_text), selector_area);

    // Statement table
    let header = Row::new([
        t(app.locale, "header.date"),
        t(app.locale, "header.total"),
        t(app.locale, "header.balance"),
        t(app.locale, "header.status"),
    ])
    .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .cc_stmt.items
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
                app.locale.enum_label(s.status_label()).to_string(),
            ])
            .style(status_style)
        })
        .collect();

    let title = if cc_accounts.is_empty() {
        t(app.locale, "title.cc_statements").to_string()
    } else if app.cc_stmt.account_idx == 0 {
        format!("{} — {}", t(app.locale, "title.cc_statements"), t(app.locale, "stmt.all_accounts"))
    } else {
        let name = cc_accounts
            .get(app.cc_stmt.account_idx - 1)
            .unwrap_or(&"?");
        format!("{} — {}", t(app.locale, "title.cc_statements"), name)
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

    frame.render_stateful_widget(table, table_area, &mut app.cc_stmt.table_state);

    // Detail pane
    let detail_content = match app
        .cc_stmt.table_state
        .selected()
        .and_then(|i| app.cc_stmt.items.get(i))
    {
        Some(s) => {
            vec![
                Line::from(format!(
                    " {} | {} - {} | {}: {}",
                    s.label(),
                    s.period_start.format("%d/%m/%Y"),
                    s.period_end.format("%d/%m/%Y"),
                    t(app.locale, "detail.due"),
                    s.due_date.format("%d/%m/%Y"),
                )),
                Line::from(format!(
                    " {}: {}  {}: {}  {}: {}  {}: {}  {}: {}",
                    t(app.locale, "detail.charges"), format_brl(s.total_charges),
                    t(app.locale, "detail.credits"), format_brl(s.total_credits),
                    t(app.locale, "detail.total"), format_brl(s.statement_total),
                    t(app.locale, "detail.paid"), format_brl(s.paid_amount),
                    t(app.locale, "detail.balance"), format_brl(s.balance_due()),
                )),
                Line::from(Span::styled(
                    if app.cc_stmt.account_idx == 0 {
                        format!(" {}", t(app.locale, "hint.cc_stmt_list_all"))
                    } else {
                        format!(" {}", t(app.locale, "hint.cc_stmt_list"))
                    },
                    Style::new().fg(Color::DarkGray),
                )),
            ]
        }
        None => vec![Line::from(format!(" {}", t(app.locale, "misc.no_sel.statement")))],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title(t(app.locale, "title.statement_details"));
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
    let stmt_info = if let Some(idx) = app.cc_stmt.table_state.selected() {
        if let Some(s) = app.cc_stmt.items.get(idx) {
            let cc_accounts: Vec<&str> = app
                .accounts
                .iter()
                .filter(|a| a.has_credit_card)
                .map(|a| a.name.as_str())
                .collect();
            let account_name = if app.cc_stmt.account_idx > 0 {
                cc_accounts
                    .get(app.cc_stmt.account_idx - 1)
                    .unwrap_or(&"?")
            } else {
                t(app.locale, "stmt.all_accounts")
            };
            vec![Line::from(format!(
                " {} — {} | {} - {} | {}: {} | {}: {}",
                account_name,
                s.label(),
                s.period_start.format("%d/%m/%Y"),
                s.period_end.format("%d/%m/%Y"),
                t(app.locale, "detail.total"), format_brl(s.statement_total),
                t(app.locale, "detail.status"), app.locale.enum_label(s.status_label()),
            ))]
        } else {
            vec![Line::from(format!(" {}", t(app.locale, "misc.no_stmt_selected")))]
        }
    } else {
        vec![Line::from(format!(" {}", t(app.locale, "misc.no_stmt_selected")))]
    };

    let header_block = Block::default().borders(Borders::ALL).title(t(app.locale, "title.statement"));
    frame.render_widget(Paragraph::new(stmt_info).block(header_block), header_area);

    // Transactions table
    let table_header = Row::new([t(app.locale, "header.date"), t(app.locale, "header.description"), t(app.locale, "header.category"), t(app.locale, "header.type"), t(app.locale, "header.amount")])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .cc_stmt.detail_txns
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
                app.category_name_localized(t.category_id).to_string(),
                app.locale.enum_label(t.parsed_type().label()).to_string(),
                format_brl(t.amount),
            ])
            .style(type_style)
        })
        .collect();

    let count = app.cc_stmt.detail_txns.len();
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
            .title(format!("{} ({count})", t(app.locale, "title.transactions"))),
    )
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.cc_stmt.detail_table_state);

    // Detail pane with key guide
    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title(t(app.locale, "title.details"));
    let detail_content = vec![Line::from(Span::styled(
        format!(" {}", t(app.locale, "hint.cc_stmt_detail")),
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
        match self.cc_stmt.view {
            StatementsView::List => self.handle_cc_statements_list_key(code).await,
            StatementsView::Detail => self.handle_cc_statements_detail_key(code).await,
        }
    }

    async fn handle_cc_statements_list_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_selection(
                    &mut self.cc_stmt.table_state,
                    self.cc_stmt.items.len(),
                    -1,
                );
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_selection(
                    &mut self.cc_stmt.table_state,
                    self.cc_stmt.items.len(),
                    1,
                );
            }
            KeyCode::Char('h') | KeyCode::Char('l') => {
                let cc_count = self.accounts.iter().filter(|a| a.has_credit_card).count();
                // +1 for "All Accounts" at index 0
                cycle_index(&mut self.cc_stmt.account_idx, cc_count + 1, code);
                self.load_cc_statements().await?;
            }
            KeyCode::Enter => {
                if self.cc_stmt.account_idx == 0 {
                    self.status_message =
                        Some(StatusMessage::info(t(self.locale, "msg.select_account_view")));
                } else if let Some(idx) = self.cc_stmt.table_state.selected()
                    && idx < self.cc_stmt.items.len()
                {
                    self.load_cc_statement_detail(idx).await?;
                    self.cc_stmt.view = StatementsView::Detail;
                }
            }
            KeyCode::Char('p') => {
                if self.cc_stmt.account_idx == 0 {
                    self.status_message =
                        Some(StatusMessage::info(t(self.locale, "msg.select_account_pay")));
                } else if let Some(stmt) = self
                    .cc_stmt.table_state
                    .selected()
                    .and_then(|i| self.cc_stmt.items.get(i))
                {
                    if stmt.is_upcoming {
                        self.status_message =
                            Some(StatusMessage::error(t(self.locale, "msg.cannot_pay_upcoming")));
                    } else if stmt.is_current {
                        self.status_message =
                            Some(StatusMessage::error(t(self.locale, "msg.cannot_pay_open")));
                    } else if stmt.balance_due() == Decimal::ZERO {
                        self.status_message =
                            Some(StatusMessage::info(t(self.locale, "msg.already_paid")));
                    } else {
                        let cc_accounts: Vec<&crate::models::Account> =
                            self.accounts.iter().filter(|a| a.has_credit_card).collect();
                        if let Some(account) = cc_accounts.get(self.cc_stmt.account_idx - 1) {
                            let balance = stmt.balance_due();
                            let label = stmt.label();
                            let pay_date = stmt.period_end.succ_opt().unwrap();
                            self.confirm_action = Some(crate::ui::app::ConfirmAction::PayCreditCardStatement {
                                account_id: account.id,
                                amount: balance,
                                date: pay_date,
                                description: format!("{} {}", t(self.locale, "title.statement"), label),
                            });
                            self.confirm_popup = Some(crate::ui::components::popup::ConfirmPopup::new(
                                crate::ui::i18n::tf_pay_statement(self.locale, &label, &crate::ui::components::format::format_brl(balance)),
                            ));
                        }
                    }
                }
            }
            KeyCode::Char('u') => {
                if self.cc_stmt.account_idx == 0 {
                    self.status_message =
                        Some(StatusMessage::info(t(self.locale, "msg.select_account_unpay")));
                } else if let Some((idx, stmt)) = self
                    .cc_stmt.table_state
                    .selected()
                    .and_then(|i| self.cc_stmt.items.get(i).map(|s| (i, s)))
                {
                    if stmt.is_upcoming || stmt.is_current {
                        self.status_message =
                            Some(StatusMessage::error(t(self.locale, "msg.only_closed_unpay")));
                    } else if stmt.paid_amount == Decimal::ZERO {
                        self.status_message =
                            Some(StatusMessage::info(t(self.locale, "msg.no_payments")));
                    } else {
                        let cc_accounts: Vec<&crate::models::Account> =
                            self.accounts.iter().filter(|a| a.has_credit_card).collect();
                        if let Some(account) = cc_accounts.get(self.cc_stmt.account_idx - 1) {
                            let pay_start = stmt.period_end.succ_opt().unwrap();
                            let pay_end = if idx > 0 {
                                let prev = &self.cc_stmt.items[idx - 1];
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
                                crate::ui::i18n::tf_unpay_statement(self.locale, &label, &crate::ui::components::format::format_brl(paid)),
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
                    &mut self.cc_stmt.detail_table_state,
                    self.cc_stmt.detail_txns.len(),
                    -1,
                );
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_selection(
                    &mut self.cc_stmt.detail_table_state,
                    self.cc_stmt.detail_txns.len(),
                    1,
                );
            }
            KeyCode::Enter => {
                if let Some(txn) = self
                    .cc_stmt.detail_table_state
                    .selected()
                    .and_then(|i| self.cc_stmt.detail_txns.get(i))
                {
                    if let Some(ip_id) = txn.installment_purchase_id {
                        // Navigate to Installments screen and select the parent purchase
                        self.screen = Screen::Installments;
                        if let Some(pos) = self.inst.items.iter().position(|ip| ip.id == ip_id) {
                            self.inst.table_state.select(Some(pos));
                        }
                    } else {
                        // Regular transaction — open edit form on Transactions screen
                        let txn = txn.clone();
                        self.txn.form = Some(TransactionForm::new_edit(
                            &txn,
                            &self.accounts,
                            &self.categories,
                            self.locale,
                        ));
                        self.screen = Screen::Transactions;
                        self.input_mode = InputMode::Editing;
                        self.load_transactions().await?;
                    }
                    self.cc_stmt.view = StatementsView::List;
                    self.cc_stmt.detail_txns.clear();
                }
            }
            KeyCode::Esc => {
                self.cc_stmt.view = StatementsView::List;
                self.cc_stmt.detail_txns.clear();
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn load_cc_statements(&mut self) -> anyhow::Result<()> {
        let cc_accounts: Vec<&Account> =
            self.accounts.iter().filter(|a| a.has_credit_card).collect();

        if self.cc_stmt.account_idx == 0 {
            // "All Accounts" — aggregate across all CC accounts by closing month
            self.cc_stmt.items = build_all_accounts_statements(&self.pool, &cc_accounts).await?;
            // Select the current/open statement
            let current_idx = self
                .cc_stmt.items
                .iter()
                .position(|s| s.is_current)
                .unwrap_or(0);
            if !self.cc_stmt.items.is_empty() {
                self.cc_stmt.table_state
                    .select(Some(current_idx.min(self.cc_stmt.items.len() - 1)));
            }
        } else if let Some(account) = cc_accounts.get(self.cc_stmt.account_idx - 1) {
            let (stmts, current_idx) = build_statements(&self.pool, account, 12).await?;
            self.cc_stmt.items = stmts;
            // Default selection to the current/open statement
            if !self.cc_stmt.items.is_empty() {
                self.cc_stmt.table_state
                    .select(Some(current_idx.min(self.cc_stmt.items.len() - 1)));
            }
        } else {
            self.cc_stmt.items.clear();
        }
        clamp_selection(&mut self.cc_stmt.table_state, self.cc_stmt.items.len());
        self.cc_stmt.view = StatementsView::List;
        self.cc_stmt.detail_txns.clear();
        Ok(())
    }

    async fn load_cc_statement_detail(&mut self, stmt_idx: usize) -> anyhow::Result<()> {
        if let Some(stmt) = self.cc_stmt.items.get(stmt_idx) {
            let cc_accounts: Vec<&Account> =
                self.accounts.iter().filter(|a| a.has_credit_card).collect();
            if let Some(account) = cc_accounts.get(self.cc_stmt.account_idx - 1) {
                let end = if stmt.is_current {
                    Local::now().date_naive()
                } else {
                    stmt.period_end
                };
                self.cc_stmt.detail_txns = db::transactions::list_credit_by_account(
                    &self.pool,
                    account.id,
                    stmt.period_start,
                    end,
                )
                .await?;
                self.cc_stmt.detail_table_state.select(
                    if self.cc_stmt.detail_txns.is_empty() {
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
        self.refresh_balances().await?;
        self.load_cc_statements().await?;
        self.refresh_dashboard_statements().await?;
        self.status_message = Some(StatusMessage::info(t(self.locale, "msg.statement_paid")));
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
        self.refresh_balances().await?;
        self.load_cc_statements().await?;
        self.refresh_dashboard_statements().await?;
        self.status_message = Some(StatusMessage::info(
            crate::ui::i18n::tf_removed_payments(self.locale, deleted)
        ));
        Ok(())
    }
}
