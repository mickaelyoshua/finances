use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Appends the form error block (blank line + red "Error: ...") if an error is present.
pub fn push_form_error<'a>(lines: &mut Vec<Line<'a>>, error: &Option<String>) {
    if let Some(err) = error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Error: {err}"),
            Style::new().fg(Color::Red),
        )));
    }
}

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

/// Renders a cycling selector field, showing a grayed-out placeholder when the list is empty.
pub fn render_selector<'a>(
    label: &str,
    options: &[&'a str],
    selected: usize,
    active: bool,
    empty_hint: &str,
) -> Line<'a> {
    if options.is_empty() {
        Line::from(Span::styled(
            format!(" {label}: ({empty_hint})"),
            Style::new().fg(Color::DarkGray),
        ))
    } else {
        render_toggle(label, options, selected, active)
    }
}
