//! Reports screen: three views (expenses by category, income by category,
//! monthly trend) with period/account/payment-method filters and HTML export.

use chrono::{Local, NaiveDate};
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph, Wrap},
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tracing::info;

use crate::{
    models::{
        CategoryAggregate, MonthlyAggregate, PaymentMethod, PeriodPreset, ReportView,
    },
    ui::{
        App,
        app::{InputMode, StatusMessage, is_toggle_key},
        components::{format::format_brl, input::InputField},
        i18n::{Locale, t},
        screens::transactions::cycle_option,
    },
};

/// Month abbreviations indexed by month number 1..=12.
const MONTH_ABBR_EN: [&str; 13] = [
    "", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTH_ABBR_PT: [&str; 13] = [
    "", "Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez",
];

pub fn month_abbr(locale: Locale, month: u32) -> &'static str {
    let m = (month as usize).min(12);
    match locale {
        Locale::En => MONTH_ABBR_EN[m],
        Locale::Pt => MONTH_ABBR_PT[m],
    }
}

// ── Filter draft (in-progress edits in the filter popup) ─────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFilterField {
    Preset,
    Start,
    End,
    Account,
    PaymentMethod,
}

impl ReportFilterField {
    const ALL: [Self; 5] = [
        Self::Preset,
        Self::Start,
        Self::End,
        Self::Account,
        Self::PaymentMethod,
    ];
}

pub struct ReportFilterDraft {
    pub preset_idx: usize,
    pub start: InputField,
    pub end: InputField,
    /// `None` = all accounts; `Some(idx)` = accounts[idx].
    pub account_idx: Option<usize>,
    /// `None` = all methods; `Some(idx)` into `PAYMENT_METHODS`.
    pub payment_method_idx: Option<usize>,
    pub active_field: usize,
}

pub const PAYMENT_METHODS: [PaymentMethod; 6] = [
    PaymentMethod::Pix,
    PaymentMethod::Credit,
    PaymentMethod::Debit,
    PaymentMethod::Cash,
    PaymentMethod::Boleto,
    PaymentMethod::Transfer,
];

impl ReportFilterDraft {
    pub fn from_app(app: &App) -> Self {
        let preset_idx = PeriodPreset::ALL
            .iter()
            .position(|p| *p == app.reports.filter.preset)
            .unwrap_or(0);
        let account_idx = app
            .reports
            .filter
            .account_id
            .and_then(|id| app.accounts.iter().position(|a| a.id == id));
        let payment_method_idx = app
            .reports
            .filter
            .payment_method
            .and_then(|m| PAYMENT_METHODS.iter().position(|x| *x == m));

        Self {
            preset_idx,
            start: InputField::new(t(app.locale, "form.from"))
                .with_value(app.reports.filter.start.format("%d-%m-%Y").to_string()),
            end: InputField::new(t(app.locale, "form.to"))
                .with_value(app.reports.filter.end.format("%d-%m-%Y").to_string()),
            account_idx,
            payment_method_idx,
            active_field: 0,
        }
    }

    pub fn active_field_id(&self) -> ReportFilterField {
        ReportFilterField::ALL[self.active_field.min(ReportFilterField::ALL.len() - 1)]
    }

    pub fn current_preset(&self) -> PeriodPreset {
        PeriodPreset::ALL[self.preset_idx.min(PeriodPreset::ALL.len() - 1)]
    }
}

// ── Rendering ────────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    // If the filter popup is active, render it overlayed on top of the view.
    let draft_active = app.reports.filter_draft.is_some();

    let [header_area, tabs_area, body_area, hint_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    render_header(frame, header_area, app);
    render_view_tabs(frame, tabs_area, app);

    if draft_active {
        render_filter_popup(frame, body_area, app);
    } else {
        match app.reports.view {
            ReportView::ExpensesByCategory => render_category_breakdown(
                frame,
                body_area,
                app,
                &app.reports.expense_by_category,
                t(app.locale, "title.expenses_by_category"),
                Color::Red,
            ),
            ReportView::IncomeByCategory => render_category_breakdown(
                frame,
                body_area,
                app,
                &app.reports.income_by_category,
                t(app.locale, "title.income_by_category"),
                Color::Green,
            ),
            ReportView::MonthlyTrend => render_monthly_trend(frame, body_area, app),
        }
    }

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", t(app.locale, "hint.reports")),
            Style::new().fg(Color::DarkGray),
        ))),
        hint_area,
    );
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let l = app.locale;
    let filter = &app.reports.filter;

    let period_label = t(l, filter.preset.i18n_key());
    let range = format!(
        "{} — {}",
        filter.start.format("%d/%m/%Y"),
        filter.end.format("%d/%m/%Y")
    );
    let account_label = filter
        .account_id
        .and_then(|id| app.accounts.iter().find(|a| a.id == id))
        .map(|a| a.name.as_str())
        .unwrap_or(t(l, "report.filter.all_accounts"));
    let method_label = filter
        .payment_method
        .map(|m| l.enum_label(m.label()))
        .unwrap_or(t(l, "report.filter.all_methods"));

    let (total_income, total_expense) =
        app.reports.monthly.iter().fold((Decimal::ZERO, Decimal::ZERO), |acc, m| {
            (acc.0 + m.income, acc.1 + m.expense)
        });

    let line1 = Line::from(vec![
        Span::styled(
            format!(" {}: ", t(l, "header.period")),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{period_label}  "),
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(range, Style::new().fg(Color::White)),
    ]);
    let line2 = Line::from(vec![
        Span::styled(
            format!(" {}: ", t(l, "header.filters")),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(account_label.to_string(), Style::new().fg(Color::White)),
        Span::raw(" · "),
        Span::styled(method_label.to_string(), Style::new().fg(Color::White)),
    ]);
    let line3 = Line::from(vec![
        Span::styled(
            format!(" {}: ", t(l, "misc.total_income")),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(format_brl(total_income), Style::new().fg(Color::Green)),
        Span::raw("   "),
        Span::styled(
            format!("{}: ", t(l, "misc.total_expense")),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(format_brl(total_expense), Style::new().fg(Color::Red)),
        Span::raw("   "),
        Span::styled(
            format!("{}: ", t(l, "misc.net")),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(
            format_brl(total_income - total_expense),
            Style::new().fg(if total_income - total_expense >= Decimal::ZERO {
                Color::Green
            } else {
                Color::Red
            }),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::new().fg(Color::DarkGray));
    frame.render_widget(
        Paragraph::new(vec![line1, line2, line3]).block(block),
        area,
    );
}

fn render_view_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let l = app.locale;
    let mut spans: Vec<Span> = Vec::new();
    for (i, view) in ReportView::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" | ", Style::new().fg(Color::DarkGray)));
        }
        let label = t(l, view.i18n_key());
        let style = if *view == app.reports.view {
            Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!(" {label} "), style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_category_breakdown(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    data: &[CategoryAggregate],
    title: &str,
    bar_color: Color,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title.to_string());

    if data.is_empty() {
        let msg = t(app.locale, "report.no_data");
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {msg}"),
                Style::new().fg(Color::DarkGray),
            )))
            .block(block),
            area,
        );
        return;
    }

    let total: Decimal = data.iter().map(|c| c.total).sum();
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows_available = inner.height as usize;
    let name_width = 16usize;
    let amount_width = 16usize;
    let pct_width = 6usize;
    // Remaining space for the bar column, with some padding.
    let bar_width = (inner.width as usize)
        .saturating_sub(name_width + amount_width + pct_width + 4)
        .max(6);

    let start = app.reports.scroll.min(data.len().saturating_sub(1));
    let end = (start + rows_available).min(data.len());

    let mut lines: Vec<Line> = Vec::with_capacity(end - start);
    for agg in &data[start..end] {
        let name = app.category_name_localized(agg.category_id);
        let truncated = truncate(name, name_width);
        let pct = if total > Decimal::ZERO {
            (agg.total / total * Decimal::from(100))
                .to_f64()
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let filled = ((pct / 100.0) * bar_width as f64).round() as usize;
        let filled = filled.min(bar_width);
        let empty = bar_width - filled;

        lines.push(Line::from(vec![
            Span::raw(format!(" {truncated:<w$} ", truncated = truncated, w = name_width)),
            Span::styled("█".repeat(filled), Style::new().fg(bar_color)),
            Span::styled("░".repeat(empty), Style::new().fg(Color::DarkGray)),
            Span::raw(format!(" {:>5.1}%", pct)),
            Span::raw(format!(" {:>w$}", format_brl(agg.total), w = amount_width)),
        ]));
    }

    if end < data.len() {
        lines.push(Line::from(Span::styled(
            format!(
                " … {} {} ({}/{})",
                data.len() - end,
                t(app.locale, "misc.more"),
                end,
                data.len()
            ),
            Style::new().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_monthly_trend(frame: &mut Frame, area: Rect, app: &App) {
    let l = app.locale;
    let block = Block::default()
        .borders(Borders::ALL)
        .title(t(l, "title.monthly_trend").to_string());

    if app.reports.monthly.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" {}", t(l, "report.no_data")),
                Style::new().fg(Color::DarkGray),
            )))
            .block(block),
            area,
        );
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Reserve 2 + N rows at the bottom for a numeric summary table (header + data rows).
    let table_rows = (app.reports.monthly.len() as u16 + 2).min(inner.height.saturating_sub(6));
    let [charts_area, table_area] = Layout::vertical([
        Constraint::Min(6),
        Constraint::Length(table_rows),
    ])
    .areas(inner);

    // Split charts area: income on left, expense on right.
    let [income_area, expense_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(charts_area);

    // Compute shared y-max so the two charts share scale.
    let max_val = app
        .reports
        .monthly
        .iter()
        .flat_map(|m| [m.income, m.expense])
        .max()
        .unwrap_or(Decimal::ZERO);
    let max_cents: u64 = (max_val * Decimal::from(100))
        .round()
        .to_u64()
        .unwrap_or(0)
        .max(1);

    render_month_bars(
        frame,
        income_area,
        &app.reports.monthly,
        l,
        t(l, "misc.total_income"),
        Color::Green,
        true,
        max_cents,
    );
    render_month_bars(
        frame,
        expense_area,
        &app.reports.monthly,
        l,
        t(l, "misc.total_expense"),
        Color::Red,
        false,
        max_cents,
    );

    render_monthly_table(frame, table_area, &app.reports.monthly, l);
}

fn render_monthly_table(
    frame: &mut Frame,
    area: Rect,
    monthly: &[MonthlyAggregate],
    locale: Locale,
) {
    if area.height == 0 {
        return;
    }
    let month_w = 8usize;
    let col_w = 16usize;
    let header = Line::from(vec![
        Span::styled(
            format!(" {:<month_w$}", t(locale, "header.period"), month_w = month_w),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{:>col_w$}", t(locale, "misc.total_income"), col_w = col_w),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{:>col_w$}", t(locale, "misc.total_expense"), col_w = col_w),
            Style::new().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{:>col_w$}", t(locale, "misc.net"), col_w = col_w),
            Style::new().fg(Color::DarkGray),
        ),
    ]);

    let mut lines = vec![header];
    for m in monthly {
        let label = format!("{} {}", month_abbr(locale, m.month), m.year % 100);
        let net = m.net();
        let net_color = if net >= Decimal::ZERO {
            Color::Green
        } else {
            Color::Red
        };
        lines.push(Line::from(vec![
            Span::raw(format!(" {:<month_w$}", label, month_w = month_w)),
            Span::styled(
                format!("{:>col_w$}", format_brl(m.income), col_w = col_w),
                Style::new().fg(Color::Green),
            ),
            Span::styled(
                format!("{:>col_w$}", format_brl(m.expense), col_w = col_w),
                Style::new().fg(Color::Red),
            ),
            Span::styled(
                format!("{:>col_w$}", format_brl(net), col_w = col_w),
                Style::new().fg(net_color),
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

#[allow(clippy::too_many_arguments)]
fn render_month_bars(
    frame: &mut Frame,
    area: Rect,
    monthly: &[MonthlyAggregate],
    locale: Locale,
    title: &str,
    color: Color,
    income: bool,
    max_cents: u64,
) {
    let bars: Vec<Bar> = monthly
        .iter()
        .map(|m| {
            let value = if income { m.income } else { m.expense };
            let cents = (value * Decimal::from(100))
                .round()
                .to_u64()
                .unwrap_or(0);
            let label: Line<'_> = month_abbr(locale, m.month).into();
            let text_value = format_brl(value);
            Bar::default()
                .value(cents)
                .label(label)
                .text_value(text_value)
                .style(Style::new().fg(color))
                .value_style(Style::new().fg(Color::Black).bg(color))
        })
        .collect();

    let group = BarGroup::default().bars(&bars);
    let chart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title.to_string()),
        )
        .data(group)
        .bar_width(4)
        .bar_gap(1)
        .max(max_cents)
        .bar_style(Style::new().fg(color))
        .label_style(Style::new().fg(Color::DarkGray))
        .bar_set(symbols::bar::NINE_LEVELS);
    frame.render_widget(chart, area);
}

fn render_filter_popup(frame: &mut Frame, area: Rect, app: &App) {
    let l = app.locale;
    let Some(draft) = &app.reports.filter_draft else { return };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(t(l, "title.report_filters").to_string())
        .border_style(Style::new().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // preset
            Constraint::Length(1), // start
            Constraint::Length(1), // end
            Constraint::Length(2), // account
            Constraint::Length(2), // payment method
            Constraint::Min(0),    // hints
        ])
        .split(inner);

    let active = draft.active_field_id();

    // Preset row
    let preset_spans = field_row_spans(
        t(l, "form.period"),
        active == ReportFilterField::Preset,
        PeriodPreset::ALL
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let label = t(l, p.i18n_key());
                let style = if i == draft.preset_idx {
                    Style::new().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::new().fg(Color::DarkGray)
                };
                Span::styled(format!(" {label} "), style)
            })
            .collect(),
    );
    frame.render_widget(
        Paragraph::new(Line::from(preset_spans)).wrap(Wrap { trim: false }),
        chunks[0],
    );

    // Start/end (only editable when Custom)
    let is_custom = draft.current_preset() == PeriodPreset::Custom;
    frame.render_widget(
        Paragraph::new(draft.start.render_line(active == ReportFilterField::Start && is_custom)),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(draft.end.render_line(active == ReportFilterField::End && is_custom)),
        chunks[2],
    );

    // Account selector
    let mut account_options: Vec<Span> = vec![selector_span(
        t(l, "report.filter.all_accounts"),
        draft.account_idx.is_none(),
    )];
    for (i, acc) in app.accounts.iter().enumerate() {
        account_options.push(selector_span(&acc.name, draft.account_idx == Some(i)));
    }
    frame.render_widget(
        Paragraph::new(Line::from(field_row_spans(
            t(l, "form.account"),
            active == ReportFilterField::Account,
            account_options,
        )))
        .wrap(Wrap { trim: false }),
        chunks[3],
    );

    // Payment method selector
    let mut pm_options: Vec<Span> = vec![selector_span(
        t(l, "report.filter.all_methods"),
        draft.payment_method_idx.is_none(),
    )];
    for (i, m) in PAYMENT_METHODS.iter().enumerate() {
        pm_options.push(selector_span(
            l.enum_label(m.label()),
            draft.payment_method_idx == Some(i),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(field_row_spans(
            t(l, "form.payment"),
            active == ReportFilterField::PaymentMethod,
            pm_options,
        )))
        .wrap(Wrap { trim: false }),
        chunks[4],
    );

    // Hints
    let hints = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(
                " Tab/↑↓: {}    Space/←→: {}    Enter: {}    Esc: {}",
                t(l, "status.nav_fields"),
                t(l, "status.cycle"),
                t(l, "status.apply"),
                t(l, "status.close"),
            ),
            Style::new().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(hints), chunks[5]);
}

fn field_row_spans<'a>(label: &'a str, active: bool, options: Vec<Span<'a>>) -> Vec<Span<'a>> {
    let label_style = if active {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::DarkGray)
    };
    let mut spans = vec![Span::styled(format!(" {label}: "), label_style)];
    for (i, opt) in options.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(opt);
    }
    spans
}

fn selector_span<'a>(text: &'a str, selected: bool) -> Span<'a> {
    let style = if selected {
        Style::new().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::new().fg(Color::DarkGray)
    };
    Span::styled(format!(" {text} "), style)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

// ── Key handlers (impl on App) ────────────────────────────────────────

impl App {
    pub(crate) async fn handle_reports_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Tab => {
                self.reports.view = self.reports.view.cycle_next();
                self.reports.scroll = 0;
            }
            KeyCode::BackTab => {
                self.reports.view = self.reports.view.cycle_prev();
                self.reports.scroll = 0;
            }
            KeyCode::Char('f') => {
                self.reports.filter_draft = Some(ReportFilterDraft::from_app(self));
                self.input_mode = InputMode::Filtering;
            }
            KeyCode::Char('p') => {
                let today = Local::now().date_naive();
                let next = self.reports.filter.preset.cycle_next();
                self.reports.filter.apply_preset(next, today);
                self.refresh_reports().await?;
            }
            KeyCode::Char('e') => {
                self.export_report().await?;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.current_category_list_len().saturating_sub(1);
                if self.reports.scroll < max {
                    self.reports.scroll += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.reports.scroll = self.reports.scroll.saturating_sub(1);
            }
            _ => {}
        }
        Ok(())
    }

    fn current_category_list_len(&self) -> usize {
        match self.reports.view {
            ReportView::ExpensesByCategory => self.reports.expense_by_category.len(),
            ReportView::IncomeByCategory => self.reports.income_by_category.len(),
            ReportView::MonthlyTrend => 0,
        }
    }

    pub(crate) async fn handle_reports_filter_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.reports.filter_draft = None;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                self.apply_reports_filter_draft().await?;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Tab | KeyCode::Down => {
                if let Some(draft) = &mut self.reports.filter_draft
                    && draft.active_field < ReportFilterField::ALL.len() - 1
                {
                    draft.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if let Some(draft) = &mut self.reports.filter_draft
                    && draft.active_field > 0
                {
                    draft.active_field -= 1;
                }
            }
            _ => {
                if let Some(draft) = &mut self.reports.filter_draft {
                    let field = draft.active_field_id();
                    match field {
                        ReportFilterField::Preset => {
                            if is_toggle_key(key.code) {
                                let len = PeriodPreset::ALL.len();
                                match key.code {
                                    KeyCode::Left => {
                                        draft.preset_idx =
                                            (draft.preset_idx + len - 1) % len;
                                    }
                                    _ => {
                                        draft.preset_idx = (draft.preset_idx + 1) % len;
                                    }
                                }
                            }
                        }
                        ReportFilterField::Start => {
                            if draft.current_preset() == PeriodPreset::Custom {
                                draft.start.handle_key(key.code);
                            }
                        }
                        ReportFilterField::End => {
                            if draft.current_preset() == PeriodPreset::Custom {
                                draft.end.handle_key(key.code);
                            }
                        }
                        ReportFilterField::Account => {
                            if is_toggle_key(key.code) {
                                let len = self.accounts.len();
                                draft.account_idx =
                                    cycle_option(draft.account_idx, len, key.code);
                            }
                        }
                        ReportFilterField::PaymentMethod => {
                            if is_toggle_key(key.code) {
                                draft.payment_method_idx = cycle_option(
                                    draft.payment_method_idx,
                                    PAYMENT_METHODS.len(),
                                    key.code,
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn apply_reports_filter_draft(&mut self) -> anyhow::Result<()> {
        let Some(draft) = self.reports.filter_draft.take() else {
            return Ok(());
        };
        let today = Local::now().date_naive();
        let preset = draft.current_preset();
        self.reports.filter.preset = preset;

        if preset == PeriodPreset::Custom {
            let start = NaiveDate::parse_from_str(draft.start.value.trim(), "%d-%m-%Y");
            let end = NaiveDate::parse_from_str(draft.end.value.trim(), "%d-%m-%Y");
            match (start, end) {
                (Ok(s), Ok(e)) if s <= e => {
                    self.reports.filter.start = s;
                    self.reports.filter.end = e;
                }
                _ => {
                    self.status_message = Some(StatusMessage::error(
                        t(self.locale, "err.invalid_date_range").to_string(),
                    ));
                }
            }
        } else if let Some((s, e)) = preset.resolve(today) {
            self.reports.filter.start = s;
            self.reports.filter.end = e;
        }

        self.reports.filter.account_id = draft
            .account_idx
            .and_then(|i| self.accounts.get(i))
            .map(|a| a.id);
        self.reports.filter.payment_method =
            draft.payment_method_idx.and_then(|i| PAYMENT_METHODS.get(i).copied());

        self.refresh_reports().await?;
        Ok(())
    }

    async fn export_report(&mut self) -> anyhow::Result<()> {
        match crate::export::export_report_html(self) {
            Ok(path) => {
                info!(?path, "report exported");
                self.status_message = Some(StatusMessage::info(format!(
                    "{}: {}",
                    t(self.locale, "status.export_report"),
                    path.display()
                )));
            }
            Err(e) => {
                self.status_message =
                    Some(StatusMessage::error(format!("export failed: {e}")));
            }
        }
        Ok(())
    }
}
