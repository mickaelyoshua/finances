//! CSV export for all entity types.
//!
//! Files are written to `~/.local/share/finances/exports/` with timestamped names.
//! Amounts are raw decimals (e.g., `1234.56`), not BRL-formatted, so the output
//! is directly importable into spreadsheets without locale-aware parsing.
//!
//! Export functions take closures for ID→name resolution (`account_id → "Nubank"`)
//! because models store foreign-key IDs, not denormalized names.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{Context, Result};
use chrono::Local;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tracing::info;

use crate::models::{
    Account, Budget, Category, CategoryAggregate, CreditCardPayment, MonthlyAggregate,
    RecurringTransaction, ReportView, Transaction, Transfer,
};
use crate::ui::App;
use crate::ui::components::format::format_brl;
use crate::ui::i18n::t;
use crate::ui::screens::reports::month_abbr;

/// Monotonic counter to guarantee unique filenames even when called within the same millisecond.
static EXPORT_SEQ: AtomicU32 = AtomicU32::new(0);

fn export_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .context("could not resolve data directory")?
        .join("finances")
        .join("exports");
    std::fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

fn export_path(name: &str) -> Result<PathBuf> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let seq = EXPORT_SEQ.fetch_add(1, Ordering::Relaxed);
    Ok(export_dir()?.join(format!("{name}_{timestamp}_{seq}.csv")))
}

fn export_path_ext(name: &str, ext: &str) -> Result<PathBuf> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let seq = EXPORT_SEQ.fetch_add(1, Ordering::Relaxed);
    Ok(export_dir()?.join(format!("{name}_{timestamp}_{seq}.{ext}")))
}

pub fn export_transactions(
    txns: &[Transaction],
    account_name: impl Fn(i32) -> String,
    category_name: impl Fn(i32) -> String,
) -> Result<PathBuf> {
    let path = export_path("transactions")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record([
        "Date",
        "Description",
        "Amount",
        "Type",
        "Payment Method",
        "Account",
        "Category",
    ])?;
    for t in txns {
        wtr.write_record([
            t.date.to_string(),
            t.description.clone(),
            t.amount.to_string(),
            t.parsed_type().label().to_string(),
            t.parsed_payment_method().label().to_string(),
            account_name(t.account_id),
            category_name(t.category_id),
        ])?;
    }
    wtr.flush()?;
    info!(rows = txns.len(), path = %path.display(), "exported transactions");
    Ok(path)
}

pub fn export_transfers(
    transfers: &[Transfer],
    account_name: impl Fn(i32) -> String,
) -> Result<PathBuf> {
    let path = export_path("transfers")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record([
        "Date",
        "From Account",
        "To Account",
        "Amount",
        "Description",
    ])?;
    for t in transfers {
        wtr.write_record([
            t.date.to_string(),
            account_name(t.from_account_id),
            account_name(t.to_account_id),
            t.amount.to_string(),
            t.description.clone(),
        ])?;
    }
    wtr.flush()?;
    info!(rows = transfers.len(), path = %path.display(), "exported transfers");
    Ok(path)
}

pub fn export_cc_payments(
    payments: &[CreditCardPayment],
    account_name: impl Fn(i32) -> String,
) -> Result<PathBuf> {
    let path = export_path("cc_payments")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record(["Date", "Account", "Amount", "Description"])?;
    for p in payments {
        wtr.write_record([
            p.date.to_string(),
            account_name(p.account_id),
            p.amount.to_string(),
            p.description.clone(),
        ])?;
    }
    wtr.flush()?;
    info!(rows = payments.len(), path = %path.display(), "exported cc_payments");
    Ok(path)
}

pub fn export_recurring(
    recurring: &[RecurringTransaction],
    account_name: impl Fn(i32) -> String,
    category_name: impl Fn(i32) -> String,
) -> Result<PathBuf> {
    let path = export_path("recurring")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record([
        "Description",
        "Amount",
        "Type",
        "Frequency",
        "Next Due",
        "Account",
        "Payment Method",
        "Category",
        "Active",
    ])?;
    for r in recurring {
        wtr.write_record([
            r.description.clone(),
            r.amount.to_string(),
            r.parsed_type().label().to_string(),
            r.parsed_frequency().label().to_string(),
            r.next_due.to_string(),
            account_name(r.account_id),
            r.parsed_payment_method().label().to_string(),
            category_name(r.category_id),
            if r.active { "Yes" } else { "No" }.to_string(),
        ])?;
    }
    wtr.flush()?;
    info!(rows = recurring.len(), path = %path.display(), "exported recurring");
    Ok(path)
}

pub fn export_accounts(accounts: &[Account]) -> Result<PathBuf> {
    let path = export_path("accounts")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record([
        "Name",
        "Type",
        "Has Credit Card",
        "Credit Limit",
        "Billing Day",
        "Due Day",
        "Has Debit Card",
        "Active",
    ])?;
    for a in accounts {
        wtr.write_record([
            a.name.clone(),
            a.parsed_type().label().to_string(),
            if a.has_credit_card { "Yes" } else { "No" }.to_string(),
            a.credit_limit.map_or(String::new(), |d| d.to_string()),
            a.billing_day.map_or(String::new(), |d| d.to_string()),
            a.due_day.map_or(String::new(), |d| d.to_string()),
            if a.has_debit_card { "Yes" } else { "No" }.to_string(),
            if a.active { "Yes" } else { "No" }.to_string(),
        ])?;
    }
    wtr.flush()?;
    info!(rows = accounts.len(), path = %path.display(), "exported accounts");
    Ok(path)
}

pub fn export_categories(categories: &[Category]) -> Result<PathBuf> {
    let path = export_path("categories")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record(["Name", "Type"])?;
    for c in categories {
        wtr.write_record([c.name.clone(), c.parsed_type().label().to_string()])?;
    }
    wtr.flush()?;
    info!(rows = categories.len(), path = %path.display(), "exported categories");
    Ok(path)
}

pub fn export_budgets(
    budgets: &[Budget],
    category_name: impl Fn(i32) -> String,
    budget_spent: &HashMap<i32, Decimal>,
) -> Result<PathBuf> {
    let path = export_path("budgets")?;
    let mut wtr = csv::Writer::from_path(&path)?;
    wtr.write_record(["Category", "Amount", "Period", "Spent", "Percentage"])?;
    for b in budgets {
        let spent = budget_spent.get(&b.id).copied().unwrap_or(Decimal::ZERO);
        let pct = if b.amount > Decimal::ZERO {
            (spent / b.amount * Decimal::from(100)).round_dp(1)
        } else {
            Decimal::ZERO
        };
        wtr.write_record([
            category_name(b.category_id),
            b.amount.to_string(),
            b.parsed_period().label().to_string(),
            spent.to_string(),
            format!("{pct}%"),
        ])?;
    }
    wtr.flush()?;
    info!(rows = budgets.len(), path = %path.display(), "exported budgets");
    Ok(path)
}

// ── HTML + SVG reports ───────────────────────────────────────────────

/// Escape HTML/XML special characters so arbitrary text (category names, account
/// names, descriptions) can be safely embedded in markup or SVG attributes.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn decimal_to_f64(d: Decimal) -> f64 {
    d.to_f64().unwrap_or(0.0)
}

/// Horizontal bar chart SVG for category breakdowns (one bar per category).
fn svg_category_breakdown(data: &[CategoryAggregate], app: &App, color: &str) -> String {
    if data.is_empty() {
        return String::new();
    }
    let total: Decimal = data.iter().map(|c| c.total).sum();
    let total_f = decimal_to_f64(total).max(1.0);

    let row_h = 28.0;
    let top = 10.0;
    let left = 180.0;
    let right_pad = 160.0;
    let width = 880.0;
    let bar_area = width - left - right_pad;
    let height = top + row_h * data.len() as f64 + 10.0;

    let mut out = String::new();
    let _ = write!(
        out,
        r##"<svg viewBox="0 0 {w} {h}" xmlns="http://www.w3.org/2000/svg" font-family="sans-serif" font-size="13">"##,
        w = width,
        h = height
    );

    for (i, agg) in data.iter().enumerate() {
        let y = top + i as f64 * row_h;
        let value_f = decimal_to_f64(agg.total);
        let pct = value_f / total_f * 100.0;
        let bar_w = (pct / 100.0) * bar_area;
        let name = escape_html(app.category_name_localized(agg.category_id));
        let amount = escape_html(&format_brl(agg.total));

        // Label on the left
        let _ = write!(
            out,
            r##"<text x="{}" y="{}" text-anchor="end" fill="#ddd">{}</text>"##,
            left - 8.0,
            y + row_h * 0.6,
            name
        );
        // Bar background
        let _ = write!(
            out,
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#2a2a2a" rx="3"/>"##,
            left,
            y + 4.0,
            bar_area,
            row_h - 8.0
        );
        // Filled bar
        let _ = write!(
            out,
            r##"<rect x="{}" y="{}" width="{:.2}" height="{}" fill="{}" rx="3"/>"##,
            left,
            y + 4.0,
            bar_w,
            row_h - 8.0,
            color
        );
        // Amount + percentage on the right
        let _ = write!(
            out,
            r##"<text x="{}" y="{}" fill="#ddd">{} · {:.1}%</text>"##,
            left + bar_area + 8.0,
            y + row_h * 0.6,
            amount,
            pct
        );
    }

    out.push_str("</svg>");
    out
}

/// Grouped bar chart SVG for monthly trend (income vs expense per month).
fn svg_monthly_trend(monthly: &[MonthlyAggregate], app: &App) -> String {
    if monthly.is_empty() {
        return String::new();
    }
    let locale = app.locale;
    let max_val = monthly
        .iter()
        .flat_map(|m| [decimal_to_f64(m.income), decimal_to_f64(m.expense)])
        .fold(0.0_f64, f64::max)
        .max(1.0);

    let width = 880.0;
    let height = 320.0;
    let top = 20.0;
    let bottom = 260.0;
    let left = 60.0;
    let right = width - 20.0;
    let plot_w = right - left;
    let plot_h = bottom - top;

    let group_w = plot_w / monthly.len() as f64;
    let bar_w = (group_w / 2.0 - 4.0).max(6.0);

    let mut out = String::new();
    let _ = write!(
        out,
        r##"<svg viewBox="0 0 {w} {h}" xmlns="http://www.w3.org/2000/svg" font-family="sans-serif" font-size="12">"##,
        w = width,
        h = height
    );

    // Axes
    let _ = write!(
        out,
        r##"<line x1="{l}" y1="{t}" x2="{l}" y2="{b}" stroke="#555"/>"##,
        l = left,
        t = top,
        b = bottom
    );
    let _ = write!(
        out,
        r##"<line x1="{l}" y1="{b}" x2="{r}" y2="{b}" stroke="#555"/>"##,
        l = left,
        b = bottom,
        r = right
    );

    for (i, m) in monthly.iter().enumerate() {
        let gx = left + i as f64 * group_w;
        let income = decimal_to_f64(m.income);
        let expense = decimal_to_f64(m.expense);
        let h_inc = income / max_val * plot_h;
        let h_exp = expense / max_val * plot_h;
        let x_inc = gx + (group_w / 2.0 - bar_w) / 2.0;
        let x_exp = gx + group_w / 2.0 + (group_w / 2.0 - bar_w) / 2.0;
        // Income bar
        let _ = write!(
            out,
            r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="#3fa34d"/>"##,
            x_inc,
            bottom - h_inc,
            bar_w,
            h_inc
        );
        // Expense bar
        let _ = write!(
            out,
            r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="#c94a4a"/>"##,
            x_exp,
            bottom - h_exp,
            bar_w,
            h_exp
        );
        // X-axis label
        let label = escape_html(&format!("{} {}", month_abbr(locale, m.month), m.year % 100));
        let _ = write!(
            out,
            r##"<text x="{:.2}" y="{:.2}" text-anchor="middle" fill="#bbb">{}</text>"##,
            gx + group_w / 2.0,
            bottom + 16.0,
            label
        );
    }

    // Legend
    let _ = write!(
        out,
        r##"<rect x="{}" y="{}" width="12" height="12" fill="#3fa34d"/><text x="{}" y="{}" fill="#ddd">{}</text>"##,
        left,
        height - 20.0,
        left + 18.0,
        height - 10.0,
        escape_html(t(locale, "misc.total_income"))
    );
    let _ = write!(
        out,
        r##"<rect x="{}" y="{}" width="12" height="12" fill="#c94a4a"/><text x="{}" y="{}" fill="#ddd">{}</text>"##,
        left + 180.0,
        height - 20.0,
        left + 198.0,
        height - 10.0,
        escape_html(t(locale, "misc.total_expense"))
    );

    out.push_str("</svg>");
    out
}

/// Render the Reports screen state as a standalone HTML file with embedded SVG charts.
/// Writes to `~/.local/share/finances/exports/` and returns the path.
pub fn export_report_html(app: &App) -> Result<PathBuf> {
    let path = export_path_ext("report", "html")?;
    let l = app.locale;
    let r = &app.reports;

    let view_title = escape_html(t(l, r.view.i18n_key()));
    let period = escape_html(t(l, r.filter.preset.i18n_key()));
    let range = format!(
        "{} — {}",
        r.filter.start.format("%d/%m/%Y"),
        r.filter.end.format("%d/%m/%Y")
    );
    let account_label = r
        .filter
        .account_id
        .and_then(|id| app.accounts.iter().find(|a| a.id == id))
        .map(|a| a.name.as_str())
        .unwrap_or(t(l, "report.filter.all_accounts"));
    let method_label = r
        .filter
        .payment_method
        .map(|m| l.enum_label(m.label()).to_string())
        .unwrap_or_else(|| t(l, "report.filter.all_methods").to_string());

    let (total_income, total_expense) = r
        .monthly
        .iter()
        .fold((Decimal::ZERO, Decimal::ZERO), |acc, m| {
            (acc.0 + m.income, acc.1 + m.expense)
        });
    let net = total_income - total_expense;

    let chart_svg = match r.view {
        ReportView::ExpensesByCategory => {
            svg_category_breakdown(&r.expense_by_category, app, "#c94a4a")
        }
        ReportView::IncomeByCategory => {
            svg_category_breakdown(&r.income_by_category, app, "#3fa34d")
        }
        ReportView::MonthlyTrend => svg_monthly_trend(&r.monthly, app),
    };

    let generated_at = Local::now().format("%Y-%m-%d %H:%M");
    let html = format!(
        r##"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<title>{title}</title>
<style>
  body {{ background:#121212; color:#e0e0e0; font-family: -apple-system, Segoe UI, sans-serif; margin:24px; }}
  h1 {{ color:#4fc3f7; margin-bottom:4px; }}
  .meta {{ color:#9e9e9e; font-size:13px; margin-bottom:16px; }}
  .totals span {{ margin-right:16px; }}
  .income {{ color:#66bb6a; }}
  .expense {{ color:#ef5350; }}
  .net.positive {{ color:#66bb6a; }}
  .net.negative {{ color:#ef5350; }}
  .chart {{ background:#1b1b1b; padding:16px; border-radius:8px; }}
  footer {{ margin-top:24px; color:#666; font-size:11px; }}
</style>
</head>
<body>
  <h1>{title}</h1>
  <div class="meta">
    <div><strong>{period_lbl}:</strong> {period} ({range})</div>
    <div><strong>{filters_lbl}:</strong> {account} · {method}</div>
    <div class="totals">
      <span class="income"><strong>{inc_lbl}:</strong> {inc}</span>
      <span class="expense"><strong>{exp_lbl}:</strong> {exp}</span>
      <span class="net {net_class}"><strong>{net_lbl}:</strong> {net_val}</span>
    </div>
  </div>
  <div class="chart">{chart}</div>
  <footer>Generated {generated_at} · finances-tui</footer>
</body>
</html>
"##,
        lang = if matches!(l, crate::ui::i18n::Locale::Pt) { "pt-BR" } else { "en" },
        title = view_title,
        period_lbl = escape_html(t(l, "header.period")),
        period = period,
        range = escape_html(&range),
        filters_lbl = escape_html(t(l, "header.filters")),
        account = escape_html(account_label),
        method = escape_html(&method_label),
        inc_lbl = escape_html(t(l, "misc.total_income")),
        inc = escape_html(&format_brl(total_income)),
        exp_lbl = escape_html(t(l, "misc.total_expense")),
        exp = escape_html(&format_brl(total_expense)),
        net_lbl = escape_html(t(l, "misc.net")),
        net_val = escape_html(&format_brl(net)),
        net_class = if net >= Decimal::ZERO { "positive" } else { "negative" },
        chart = chart_svg,
        generated_at = generated_at,
    );

    std::fs::write(&path, html)?;
    info!(path = %path.display(), "exported report HTML");
    Ok(path)
}
