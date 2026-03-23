use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::ui::{App, components::format::format_brl};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let has_notifs = !app.notifications.is_empty();
    let has_cc = !app.dashboard_current_statements.is_empty();

    let cc_height = if has_cc {
        (app.dashboard_current_statements.len() as u16 + 2).min(8) // +2 for border
    } else {
        0
    };

    let mut constraints: Vec<Constraint> = Vec::new();
    if has_notifs {
        let notif_height = (app.notifications.len() as u16 + 2).min(10);
        constraints.push(Constraint::Length(notif_height));
    }
    constraints.push(Constraint::Min(5)); // balances
    if has_cc {
        constraints.push(Constraint::Length(cc_height));
    }
    constraints.push(Constraint::Min(5)); // budget + recurring

    let areas = Layout::vertical(constraints).split(area);
    let mut idx = 0;

    if has_notifs {
        render_notifications(frame, areas[idx], app);
        idx += 1;
    }

    render_balances(frame, areas[idx], app);
    idx += 1;

    if has_cc {
        render_current_statements(frame, areas[idx], app);
        idx += 1;
    }

    let [budget_area, recurring_area] =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .areas(areas[idx]);

    render_budgets(frame, budget_area, app);
    render_recurring(frame, recurring_area, app);
}

fn render_notifications(frame: &mut Frame, area: Rect, app: &App) {
    let lines: Vec<Line> = app
        .notifications
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let style = if i == app.notification_selection {
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::White)
            };
            let marker = if i == app.notification_selection {
                "▸ "
            } else {
                "  "
            };
            Line::styled(format!("{}{}", marker, n.message), style)
        })
        .collect();

    let title = format!(
        "Notifications ({} unread) — r: dismiss  R: dismiss all",
        app.notifications.len()
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::new().fg(Color::Yellow));
    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}

fn render_balances(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(["Account", "Type", "Checking", "Credit Used"])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let balances: Vec<(Decimal, Decimal)> = app
        .accounts
        .iter()
        .map(|acc| app.balances.get(&acc.id).copied().unwrap_or_default())
        .collect();

    let total_checking: Decimal = balances.iter().map(|(c, _)| c).sum();
    let total_credit: Decimal = balances.iter().map(|(_, c)| c).sum();
    let total_limit: Decimal = app
        .accounts
        .iter()
        .filter(|acc| acc.has_credit_card)
        .map(|acc| acc.credit_limit.unwrap_or(Decimal::ZERO))
        .sum();
    let total_free = total_limit - total_credit;

    let mut rows: Vec<Row> = app
        .accounts
        .iter()
        .zip(&balances)
        .map(|(acc, &(checking, credit))| {
            let credit_cell = if acc.has_credit_card {
                let limit = acc.credit_limit.unwrap_or(Decimal::ZERO);
                let free = limit - credit;
                format!(
                    "{} / {} ({} free)",
                    format_brl(credit),
                    format_brl(limit),
                    format_brl(free)
                )
            } else {
                "-".to_string()
            };
            Row::new([
                acc.name.clone(),
                acc.parsed_type().label().to_string(),
                format_brl(checking),
                credit_cell,
            ])
        })
        .collect();

    // Totals row
    if !app.accounts.is_empty() {
        rows.push(
            Row::new([
                "TOTAL".to_string(),
                String::new(),
                format_brl(total_checking),
                format!(
                    "{} / {} ({} free)",
                    format_brl(total_credit),
                    format_brl(total_limit),
                    format_brl(total_free)
                ),
            ])
            .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        );
    }

    let table = Table::new(
        rows,
        [
            Constraint::Min(12),
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Accounts Balances"),
    );

    frame.render_widget(table, area);
}

fn render_current_statements(frame: &mut Frame, area: Rect, app: &App) {
    let lines: Vec<Line> = app
        .dashboard_current_statements
        .iter()
        .map(|(name, stmt)| {
            Line::from(vec![
                Span::styled(format!("  {:<14}", name), Style::new().fg(Color::White)),
                Span::styled(
                    format!(
                        "{} - {}",
                        stmt.period_start.format("%d/%m"),
                        stmt.period_end.format("%d/%m")
                    ),
                    Style::new().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(
                    format_brl(stmt.statement_total),
                    if stmt.statement_total > Decimal::ZERO {
                        Style::new().fg(Color::Red)
                    } else {
                        Style::new().fg(Color::Green)
                    },
                ),
                Span::styled(
                    format!("  due {}", stmt.due_date.format("%d/%m")),
                    Style::new().fg(Color::DarkGray),
                ),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Current CC Statements (Open)");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_budgets(frame: &mut Frame, area: Rect, app: &App) {
    let lines: Vec<Line> = app
        .budgets
        .iter()
        .map(|b| {
            let cat_name = app.category_name(b.category_id);

            let spent = app
                .budget_spent
                .get(&b.id)
                .copied()
                .unwrap_or(Decimal::ZERO);
            let ratio = if b.amount > Decimal::ZERO {
                (spent / b.amount).to_f64().unwrap_or(0.0)
            } else {
                0.0
            };

            let bar_width = 10;
            let filled = ((ratio * bar_width as f64).round() as usize).min(bar_width);
            let empty = bar_width - filled;

            let bar_color = if ratio >= 1.0 {
                Color::Red
            } else if ratio >= 0.75 {
                Color::Yellow
            } else {
                Color::Green
            };

            Line::from(vec![
                Span::raw(format!(" {:<12}", cat_name)),
                Span::styled("█".repeat(filled), Style::new().fg(bar_color)),
                Span::styled("░".repeat(empty), Style::new().fg(Color::DarkGray)),
                Span::raw(format!(" {} / {}", format_brl(spent), format_brl(b.amount))),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Budget Status");
    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}

fn render_recurring(frame: &mut Frame, area: Rect, app: &App) {
    let lines: Vec<Line> = if app.pending_recurring.is_empty() {
        vec![Line::from("  No pending recurring transactions.")]
    } else {
        app.pending_recurring
            .iter()
            .map(|r| {
                Line::from(format!(
                    "  {} - {} (due {})",
                    r.description,
                    format_brl(r.amount),
                    r.next_due.format("%b %d"),
                ))
            })
            .collect()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Pending Recurring");
    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}
