use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};

use super::app::{App, Screen};

pub fn draw(frame: &mut Frame, app: &App) {
    let [tab_area, content_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    render_tabs(frame, tab_area, app.screen);
    render_content(frame, content_area, app);
    render_status_bar(frame, status_area);
}

fn render_tabs(frame: &mut Frame, area: Rect, current: Screen) {
    let titles: Vec<Line> = Screen::ALL
        .iter()
        .enumerate()
        .map(|(i, s)| Line::from(format!(" {} {} ", i + 1, s.label())))
        .collect();

    let tabs = Tabs::new(titles)
        .select(current.index())
        .highlight_style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .divider("|");

    frame.render_widget(tabs, area);
}

fn render_content(frame: &mut Frame, area: Rect, app: &App) {
    match app.screen {
        Screen::Dashboard => super::screens::dashboard::render(frame, area, app),
        _ => {
            let block = Block::default()
                .borders(Borders::ALL)
                .title(app.screen.label());
            let paragraph =
                Paragraph::new(format!("{} - coming soon", app.screen.label())).block(block);

            frame.render_widget(paragraph, area);
        }
    }
}

fn render_status_bar(frame: &mut Frame, area: Rect) {
    let hints = Line::from(vec![
        Span::styled(" q", Style::new().fg(Color::Yellow)),
        Span::raw(" Quit "),
        Span::styled("1-7", Style::new().fg(Color::Yellow)),
        Span::raw(" Screen "),
        Span::styled("← →", Style::new().fg(Color::Yellow)),
        Span::raw("Navigate"),
    ]);

    frame.render_widget(Paragraph::new(hints), area);
}
