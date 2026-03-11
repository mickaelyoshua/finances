use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub fn render_toggle<'a>(
    label: &str,
    options: &[&'a str],
    selected: usize,
    active: bool,
) -> Line<'a> {
    let label_style = if active {
        Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::DarkGray)
    };

    let mut spans = vec![Span::styled(format!(" {}: ", label), label_style)];

    for (i, option) in options.iter().enumerate() {
        let style = if i == selected {
            Style::new().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::new().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!(" {} ", option), style));
        if i < options.len() - 1 {
            spans.push(Span::raw(" "));
        }
    }

    Line::from(spans)
}
