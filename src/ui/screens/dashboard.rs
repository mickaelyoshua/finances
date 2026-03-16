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
    if app.notifications.is_empty() {
        // Original 3-section layout
        let [top, bottom] =
            Layout::vertical([Constraint::Min(5), Constraint::Min(5)]).areas(area);
        let [budget_area, recurring_area] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(bottom);

        render_balances(frame, top, app);
        render_budgets(frame, budget_area, app);
        render_recurring(frame, recurring_area, app);
    } else {
        // 4-section layout: notifications at top
        let notif_height = (app.notifications.len() as u16 + 2).min(10); // +2 for border, cap at 10
        let [notif_area, top, bottom] = Layout::vertical([
            Constraint::Length(notif_height),
            Constraint::Min(5),
            Constraint::Min(5),
        ])
        .areas(area);
        let [budget_area, recurring_area] =
            Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                .areas(bottom);

        render_notifications(frame, notif_area, app);
        render_balances(frame, top, app);
        render_budgets(frame, budget_area, app);
        render_recurring(frame, recurring_area, app);
    }
}

fn render_notifications(frame: &mut Frame, area: Rect, app: &App) {
    let lines: Vec<Line> = app
        .notifications
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let style = if i == app.notification_selection {
                Style::new()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
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

    let rows: Vec<Row> = app
        .accounts
        .iter()
        .map(|acc| {
            let (checking, credit) = app.balances.get(&acc.id).copied().unwrap_or_default();
            let credit_cell = if acc.has_credit_card {
                let limit = acc.credit_limit.unwrap_or(Decimal::ZERO);
                format!("{} / {}", format_brl(credit), format_brl(limit))
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
