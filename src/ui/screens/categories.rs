use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use tracing::info;

use crate::{
    models::CategoryType,
    ui::{
        App,
        app::{
            ConfirmAction, InputMode, StatusMessage, clamp_selection, is_toggle_key,
            move_table_selection,
        },
        components::{
            input::InputField,
            popup::ConfirmPopup,
            toggle::{push_form_error, render_toggle},
        },
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

pub struct ValidatedCategory {
    pub name: String,
    pub category_type: CategoryType,
}

impl CategoryForm {
    pub fn validate(&self) -> Result<ValidatedCategory, String> {
        let name = self.name.value.trim().to_string();
        if name.is_empty() {
            return Err("Name is required".into());
        }

        Ok(ValidatedCategory {
            name,
            category_type: self.category_type,
        })
    }

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
        Layout::vertical([Constraint::Min(5), Constraint::Length(5)]).areas(area);

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
            Line::from(Span::styled(
                " [n] New  [e] Edit  [d] Delete  [x] Export",
                Style::new().fg(Color::DarkGray),
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

    push_form_error(&mut lines, &form.error);

    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

// ── Key handling & form submission (impl App) ─────────────────────

impl App {
    pub(crate) async fn handle_categories_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                move_table_selection(&mut self.category_table_state, self.categories.len(), -1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                move_table_selection(&mut self.category_table_state, self.categories.len(), 1);
            }
            KeyCode::Char('n') => {
                self.category_form = Some(CategoryForm::new_create());
                self.input_mode = InputMode::Editing;
            }
            KeyCode::Char('e') => {
                if let Some(cat) = self
                    .category_table_state
                    .selected()
                    .and_then(|i| self.categories.get(i))
                {
                    self.category_form = Some(CategoryForm::new_edit(cat));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(cat) = self
                    .category_table_state
                    .selected()
                    .and_then(|i| self.categories.get(i))
                {
                    let cat_id = cat.id;
                    let cat_name = cat.name.clone();
                    if crate::db::categories::has_references(&self.pool, cat_id).await? {
                        self.status_message = Some(StatusMessage::error(format!(
                            "Cannot delete \"{cat_name}\": category is in use"
                        )));
                    } else {
                        self.confirm_action = Some(ConfirmAction::DeleteCategory(cat_id));
                        self.confirm_popup =
                            Some(ConfirmPopup::new(format!("Delete \"{cat_name}\"?")));
                    }
                }
            }
            KeyCode::Char('x') => {
                match crate::export::export_categories(&self.categories) {
                    Ok(path) => {
                        self.status_message = Some(StatusMessage::info(format!(
                            "Exported {} categories to {}",
                            self.categories.len(),
                            path.display()
                        )));
                    }
                    Err(e) => {
                        self.status_message =
                            Some(StatusMessage::error(format!("Export failed: {e}")));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn handle_category_form_key(&mut self, code: KeyCode) {
        let form = self.category_form.as_mut().unwrap();
        match code {
            KeyCode::Tab | KeyCode::Down => {
                if form.active_field < CategoryField::ALL.len() - 1 {
                    form.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if form.active_field > 0 {
                    form.active_field -= 1;
                }
            }
            _ => match form.active_field_id() {
                CategoryField::Name => form.name.handle_key(code),
                CategoryField::CategoryType => {
                    if is_toggle_key(code) {
                        form.category_type = match form.category_type {
                            CategoryType::Expense => CategoryType::Income,
                            CategoryType::Income => CategoryType::Expense,
                        };
                    }
                }
            },
        }
    }

    pub(crate) async fn submit_category_form(&mut self) -> anyhow::Result<()> {
        use crate::db::categories;

        let form = self.category_form.as_ref().unwrap();
        let validated = match form.validate() {
            Ok(v) => v,
            Err(e) => {
                self.category_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        // Extract mode info before dropping the shared borrow
        let edit_id = match form.mode {
            CategoryFormMode::Create => None,
            CategoryFormMode::Edit(id) => Some(id),
        };

        if let Some(id) = edit_id {
            // Guard: block type change if category is referenced
            if let Some(old) = self.categories.iter().find(|c| c.id == id) {
                if old.parsed_type() != validated.category_type
                    && categories::has_references(&self.pool, id).await?
                {
                    self.category_form.as_mut().unwrap().error = Some(
                        "Cannot change type: category is referenced by transactions or budgets"
                            .into(),
                    );
                    return Ok(());
                }
            }

            categories::update_category(&self.pool, id, &validated.name, validated.category_type)
                .await?;
            info!(id, name = %validated.name, "category updated");
        } else {
            categories::create_category(&self.pool, &validated.name, validated.category_type)
                .await?;
            info!(name = %validated.name, "category created");
        }

        self.category_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    pub(crate) async fn execute_delete_category(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::categories::delete_category(&self.pool, id).await?;
        info!(id, "category deleted");
        self.load_data().await?;
        clamp_selection(&mut self.category_table_state, self.categories.len());
        Ok(())
    }
}
