//! Top-level rendering pipeline.
//!
//! Layout (top to bottom): **tab bar** → **content** → **status bar**.
//! A [`ConfirmPopup`] renders as a centered overlay on top of everything.
//!
//! The tab bar auto-scrolls when the terminal is too narrow to show all 10
//! tabs at once, keeping the active tab visible with `◀`/`▶` overflow
//! indicators on the edges.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Tabs},
};

use super::app::{App, InputMode, Screen};
use super::i18n::{Locale, t};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [tab_area, content_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    render_tabs(frame, tab_area, app.screen, app.locale);
    render_content(frame, content_area, app);
    render_status_bar(frame, status_area, app);

    // Popup overlay (renders on top of everything)
    if let Some(popup) = &app.confirm_popup {
        popup.render(frame, frame.area(), app.locale);
    }

    // Help popup overlay
    if let Some(popup) = &app.help_popup {
        popup.render(frame, frame.area(), app.locale);
    }
}

fn render_tabs(frame: &mut Frame, area: Rect, current: Screen, locale: Locale) {
    let all_titles: Vec<String> = Screen::ALL
        .iter()
        .enumerate()
        .map(|(i, s)| format!(" {} {} ", i + 1, t(locale, s.i18n_key())))
        .collect();

    // Calculate total width needed: each title + "|" dividers between them
    let total_width: usize =
        all_titles.iter().map(|t| t.len()).sum::<usize>() + all_titles.len().saturating_sub(1);
    let available = area.width as usize;

    // If everything fits, render normally
    if total_width <= available {
        let tab_lines: Vec<Line> = all_titles.into_iter().map(Line::from).collect();
        let tabs = Tabs::new(tab_lines)
            .select(current.index())
            .highlight_style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .divider("|");
        frame.render_widget(tabs, area);
        return;
    }

    // Scroll: keep the selected tab visible with context on both sides.
    // Reserve 2 chars on each side for "< " / " >" overflow indicators.
    let current_idx = current.index();
    let count = all_titles.len();

    let mut start = current_idx;
    let mut end = current_idx + 1;
    let mut width = all_titles[current_idx].len();

    // Expand window alternating right then left
    loop {
        let mut expanded = false;

        if end < count {
            let next_w = width + 1 + all_titles[end].len(); // +1 for divider
            if next_w + 4 <= available {
                // +4 reserves space for "< " and " >"
                width = next_w;
                end += 1;
                expanded = true;
            }
        }

        if start > 0 {
            let next_w = width + 1 + all_titles[start - 1].len();
            if next_w + 4 <= available {
                width = next_w;
                start -= 1;
                expanded = true;
            }
        }

        if !expanded {
            break;
        }
    }

    let has_left = start > 0;
    let has_right = end < count;

    // Build the tab bar with overflow indicators
    let visible: Vec<Line> = all_titles[start..end]
        .iter()
        .cloned()
        .map(Line::from)
        .collect();
    let sel = current_idx - start;

    // Render into a sub-area, leaving room for arrows
    let left_pad = if has_left { 2 } else { 0 };
    let right_pad = if has_right { 2 } else { 0 };

    let inner_width = area.width.saturating_sub(left_pad + right_pad);
    let tab_rect = Rect::new(area.x + left_pad, area.y, inner_width, area.height);

    let tabs = Tabs::new(visible)
        .select(sel)
        .highlight_style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .divider("|");
    frame.render_widget(tabs, tab_rect);

    // Draw overflow arrows
    if has_left {
        let arrow = Paragraph::new(Span::styled("◀ ", Style::new().fg(Color::DarkGray)));
        frame.render_widget(arrow, Rect::new(area.x, area.y, 2, 1));
    }
    if has_right {
        let arrow = Paragraph::new(Span::styled(" ▶", Style::new().fg(Color::DarkGray)));
        frame.render_widget(arrow, Rect::new(area.x + area.width - 2, area.y, 2, 1));
    }
}

fn render_content(frame: &mut Frame, area: Rect, app: &mut App) {
    match app.screen {
        Screen::Dashboard => super::screens::dashboard::render(frame, area, app),
        Screen::Accounts => super::screens::accounts::render(frame, area, app),
        Screen::Categories => super::screens::categories::render(frame, area, app),
        Screen::Transactions => super::screens::transactions::render(frame, area, app),
        Screen::Budgets => super::screens::budgets::render(frame, area, app),
        Screen::Installments => super::screens::installments::render(frame, area, app),
        Screen::Recurring => super::screens::recurring::render(frame, area, app),
        Screen::Transfers => super::screens::transfers::render(frame, area, app),
        Screen::CreditCardPayments => super::screens::cc_payments::render(frame, area, app),
        Screen::CreditCardStatements => super::screens::cc_statements::render(frame, area, app),
    }
}

fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let env_badge = if app.is_prod {
        Span::styled(
            " [PROD] ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " [DEV] ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    };

    if let Some(msg) = &app.status_message {
        let color = if msg.is_error {
            Color::Red
        } else {
            Color::Green
        };
        let line = Line::from(vec![
            env_badge,
            Span::styled(
                format!(" {}", msg.text),
                Style::new().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    let l = app.locale;
    let mut spans = vec![env_badge];
    match app.input_mode {
        InputMode::Editing => {
            spans.extend([
                Span::styled(" Esc", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {} ", t(l, "status.cancel"))),
                Span::styled("Tab/↑↓", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {} ", t(l, "status.nav_fields"))),
                Span::styled("Space", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {} ", t(l, "status.toggle"))),
                Span::styled("Enter", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {}", t(l, "status.submit"))),
            ]);
        }
        InputMode::Normal => {
            spans.extend([
                Span::styled(" q", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {} ", t(l, "status.quit"))),
                Span::styled("0-9", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {} ", t(l, "status.screen"))),
                Span::styled("← →", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {}", t(l, "status.navigate"))),
            ]);
            if !app.dashboard.notifications.is_empty() {
                spans.push(Span::styled(
                    format!(" [{} {}]", app.dashboard.notifications.len(), t(l, "status.unread")),
                    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
            }
        }
        InputMode::Filtering => {
            spans.extend([
                Span::styled(" Esc", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {} ", t(l, "status.close"))),
                Span::styled("Enter", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {} ", t(l, "status.apply"))),
                Span::styled("Tab/↑↓", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {} ", t(l, "status.nav_fields"))),
                Span::styled("Space", Style::new().fg(Color::Yellow)),
                Span::raw(format!(" {}", t(l, "status.cycle"))),
            ]);
        }
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
