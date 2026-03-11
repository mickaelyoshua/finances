use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};

use super::app::{App, InputMode, Screen};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [tab_area, content_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    render_tabs(frame, tab_area, app.screen);
    render_content(frame, content_area, app);
    render_status_bar(frame, status_area, app);

    // Popup overlay (renders on top of everything)
    if let Some(popup) = &app.confirm_popup {
        popup.render(frame, frame.area());
    }
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

fn render_content(frame: &mut Frame, area: Rect, app: &mut App) {
    match app.screen {
        Screen::Dashboard => super::screens::dashboard::render(frame, area, app),
        Screen::Accounts => super::screens::accounts::render(frame, area, app),
        Screen::Categories => super::screens::categories::render(frame, area, app),
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

fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(err) = &app.status_error {
        let line = Line::from(Span::styled(
            format!(" {err}"),
            Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    let hints = match app.input_mode {
        InputMode::Editing => Line::from(vec![
            Span::styled(" Esc", Style::new().fg(Color::Yellow)),
            Span::raw(" Cancel "),
            Span::styled("Tab/↑↓", Style::new().fg(Color::Yellow)),
            Span::raw(" Navigate "),
            Span::styled("Space", Style::new().fg(Color::Yellow)),
            Span::raw(" Toggle "),
            Span::styled("Enter", Style::new().fg(Color::Yellow)),
            Span::raw(" Submit"),
        ]),
        InputMode::Normal => Line::from(vec![
            Span::styled(" q", Style::new().fg(Color::Yellow)),
            Span::raw(" Quit "),
            Span::styled("1-7", Style::new().fg(Color::Yellow)),
            Span::raw(" Screen "),
            Span::styled("← →", Style::new().fg(Color::Yellow)),
            Span::raw(" Navigate"),
        ]),
    };

    frame.render_widget(Paragraph::new(hints), area);
}
