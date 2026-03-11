use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};

use crate::{
    models::CategoryType,
    ui::{
        App,
        components::{input::InputField, toggle::render_toggle},
    },
};

pub enum CategoryFormMode {
    Create,
    Edit(i32),
}

pub struct CategoryForm {
    pub mode: CategoryFormMode,
    pub name: InputField,
    pub category_type: CategoryType,
    pub active_field: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryField {
    Name,
    CategoryType,
}

impl CategoryField {
    pub const ALL: [Self; 2] = [Self::Name, Self::CategoryType];
}

impl CategoryForm {
    pub fn new_create() -> Self {
        Self {
            mode: CategoryFormMode::Create,
            name: InputField::new("Name"),
            category_type: CategoryType::Expense,
            active_field: 0,
            error: None,
        }
    }

    pub fn new_edit(cat: &crate::models::Category) -> Self {
        Self {
            mode: CategoryFormMode::Edit(cat.id),
            name: InputField::new("Name").with_value(&cat.name),
            category_type: cat.parsed_type(),
            active_field: 0,
            error: None,
        }
    }

    pub fn active_field_id(&self) -> CategoryField {
        CategoryField::ALL[self.active_field.min(CategoryField::ALL.len() - 1)]
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.category_form.is_some() {
        render_form(frame, area, app);
    } else {
        render_list(frame, area, app);
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let header = Row::new(["Category", "Type", "Created"])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let [table_area, detail_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(4)]).areas(area);

    let rows: Vec<Row> = app
        .categories
        .iter()
        .map(|cat| {
            Row::new([
                cat.name.clone(),
                cat.parsed_type().label().to_string(),
                cat.created_at.format("%d-%m-%Y").to_string(),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Categories"))
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.category_table_state);

    let detail_content = match app
        .category_table_state
        .selected()
        .and_then(|i| app.categories.get(i))
    {
        Some(cat) => vec![
            Line::from(format!(
                " Name: {} | Type: {}",
                cat.name,
                cat.parsed_type().label()
            )),
            Line::from(format!(
                " Created: {}",
                cat.created_at.format("%d-%m-%Y %H:%M")
            )),
        ],
        None => vec![Line::from(" No category selected.")],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("Category Details");
    let detail = Paragraph::new(detail_content).block(detail_block);

    frame.render_widget(detail, detail_area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.category_form.as_ref().unwrap();

    let title = match form.mode {
        CategoryFormMode::Create => "New Category",
        CategoryFormMode::Edit(_) => "Edit Category",
    };

    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in CategoryField::ALL.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            CategoryField::Name => form.name.render_line(active),
            CategoryField::CategoryType => render_toggle(
                "Type",
                &["Expense", "Income"],
                if form.category_type == CategoryType::Expense {
                    0
                } else {
                    1
                },
                active,
            ),
        };
        lines.push(line);
    }

    if let Some(err) = &form.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Error: {}", err),
            Style::new().fg(Color::Red),
        )));
    }

    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
