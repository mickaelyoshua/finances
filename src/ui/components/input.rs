use crossterm::event::KeyCode;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub struct InputField {
    pub label: String,
    pub value: String,
    pub cursor: usize,
}

impl InputField {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: String::new(),
            cursor: 0,
        }
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor = self.value.len();
        self
    }

    pub fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => {
                self.value.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.value.remove(self.cursor);
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor < self.value.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.value.len(),
            _ => {}
        }
    }

    pub fn render_line(&self, active: bool) -> Line {
        let label_style = if active {
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::DarkGray)
        };

        let value_display = if active {
            // Show cursor as a block character
            let (before, after) = self.value.split_at(self.cursor);
            let cursor_char = after.chars().next().unwrap_or(' ');
            let rest = if after.len() > cursor_char.len_utf8() {
                &after[cursor_char.len_utf8()..]
            } else {
                ""
            };
            vec![
                Span::styled(format!(" {}: ", self.label), label_style),
                Span::raw(before.to_string()),
                Span::styled(
                    cursor_char.to_string(),
                    Style::new().bg(Color::White).fg(Color::Black),
                ),
                Span::raw(rest.to_string()),
            ]
        } else {
            vec![
                Span::styled(format!(" {}: ", self.label), label_style),
                Span::raw(self.value.clone()),
            ]
        };

        Line::from(value_display)
    }
}
