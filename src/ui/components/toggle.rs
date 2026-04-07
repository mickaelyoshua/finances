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
    max_width: u16,
) -> Line<'a> {
    let label_style = if active {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::DarkGray)
    };

    let label_text = format!(" {}: ", label);
    let label_len = label_text.len();
    let mut spans = vec![Span::styled(label_text, label_style)];

    if options.is_empty() {
        return Line::from(spans);
    }

    // Width of each option: " text "
    let opt_w: Vec<usize> = options.iter().map(|o| o.len() + 2).collect();
    // Total width with separators between options
    let total: usize = opt_w.iter().sum::<usize>() + options.len() - 1;
    let available = (max_width as usize).saturating_sub(label_len);

    if total <= available {
        // All fit — render normally
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
    } else {
        // Scroll to keep selected visible
        let (start, end) = scroll_window(selected, &opt_w, available);

        if start > 0 {
            spans.push(Span::styled("◀ ", Style::new().fg(Color::DarkGray)));
        }

        for (i, option) in options.iter().enumerate().skip(start).take(end - start + 1) {
            let style = if i == selected {
                Style::new().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::new().fg(Color::DarkGray)
            };
            spans.push(Span::styled(format!(" {} ", option), style));
            if i < end {
                spans.push(Span::raw(" "));
            }
        }

        if end < options.len() - 1 {
            spans.push(Span::styled(" ▶", Style::new().fg(Color::DarkGray)));
        }
    }

    Line::from(spans)
}

/// Find the widest window [start..=end] around `selected` that fits in `available` width.
fn scroll_window(selected: usize, opt_w: &[usize], available: usize) -> (usize, usize) {
    let n = opt_w.len();
    let mut start = selected;
    let mut end = selected;

    let cost = |s: usize, e: usize| -> usize {
        let opts: usize = opt_w[s..=e].iter().sum();
        let seps = e - s;
        let left = if s > 0 { 2 } else { 0 };
        let right = if e < n - 1 { 2 } else { 0 };
        opts + seps + left + right
    };

    loop {
        let mut grew = false;

        if end + 1 < n && cost(start, end + 1) <= available {
            end += 1;
            grew = true;
        }

        if start > 0 && cost(start - 1, end) <= available {
            start -= 1;
            grew = true;
        }

        if !grew {
            break;
        }
    }

    (start, end)
}

/// Renders a cycling selector field, showing a grayed-out placeholder when the list is empty.
pub fn render_selector<'a>(
    label: &str,
    options: &[&'a str],
    selected: usize,
    active: bool,
    empty_hint: &str,
    max_width: u16,
) -> Line<'a> {
    if options.is_empty() {
        Line::from(Span::styled(
            format!(" {label}: ({empty_hint})"),
            Style::new().fg(Color::DarkGray),
        ))
    } else {
        render_toggle(label, options, selected, active, max_width)
    }
}
