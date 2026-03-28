use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::ui::i18n::{Locale, t};

pub struct ConfirmPopup {
    pub message: String,
    pub confirmed: bool, // true = Yes highlighted, false = No highlighted
}

impl ConfirmPopup {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            confirmed: false, // default to No (safer)
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> Option<bool> {
        match code {
            KeyCode::Left | KeyCode::Right => {
                self.confirmed = !self.confirmed;
                None // still deciding
            }
            KeyCode::Enter => Some(self.confirmed),
            KeyCode::Esc => Some(false),
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, locale: Locale) {
        let popup_area = centered_rect(40, 5, area);

        let yes_style = if self.confirmed {
            Style::new()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::DarkGray)
        };
        let no_style = if !self.confirmed {
            Style::new()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::DarkGray)
        };

        let text = vec![
            Line::from(format!(" {}", self.message)),
            Line::from(""),
            Line::from(vec![
                Span::raw("   "),
                Span::styled(format!(" {} ", t(locale, "misc.yes")), yes_style),
                Span::raw("  "),
                Span::styled(format!(" {} ", t(locale, "misc.no")), no_style),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .title(t(locale, "title.confirm"))
            .border_style(Style::new().fg(Color::Yellow));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(Paragraph::new(text).block(block), popup_area);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [vertical] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [horizontal] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(vertical);
    horizontal
}
