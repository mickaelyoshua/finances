use crossterm::event::KeyCode;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub struct InputField {
    pub label: String,
    pub value: String,
    /// Cursor position in characters (not bytes).
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
        self.cursor = self.value.chars().count();
        self
    }

    /// Convert a character-based cursor position to a byte offset into `self.value`.
    fn byte_offset(&self, char_pos: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len())
    }

    pub fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => {
                let byte_pos = self.byte_offset(self.cursor);
                self.value.insert(byte_pos, c);
                self.cursor += 1;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    let start = self.byte_offset(self.cursor);
                    let end = self.byte_offset(self.cursor + 1);
                    self.value.drain(start..end);
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.value.chars().count() {
                    let start = self.byte_offset(self.cursor);
                    let end = self.byte_offset(self.cursor + 1);
                    self.value.drain(start..end);
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor < self.value.chars().count() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.value.chars().count(),
            _ => {}
        }
    }

    pub fn render_line(&self, active: bool) -> Line<'_> {
        let label_style = if active {
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(Color::DarkGray)
        };

        let value_display = if active {
            let byte_pos = self.byte_offset(self.cursor);
            let (before, after) = self.value.split_at(byte_pos);
            let cursor_char = after.chars().next().unwrap_or(' ');
            let rest = if after.len() > cursor_char.len_utf8() {
                &after[cursor_char.len_utf8()..]
            } else {
                ""
            };
            vec![
                Span::styled(format!(" {}: ", self.label), label_style),
                Span::raw(before),
                Span::styled(
                    cursor_char.to_string(),
                    Style::new().bg(Color::White).fg(Color::Black),
                ),
                Span::raw(rest),
            ]
        } else {
            vec![
                Span::styled(format!(" {}: ", self.label), label_style),
                Span::raw(self.value.as_str()),
            ]
        };

        Line::from(value_display)
    }
}
