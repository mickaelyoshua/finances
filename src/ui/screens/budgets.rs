use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use rust_decimal::Decimal;
use tracing::info;

use crate::{
    models::{BudgetPeriod, Category, CategoryType},
    ui::{
        App,
        app::{
            ConfirmAction, InputMode, StatusMessage, clamp_selection, cycle_index, is_toggle_key,
            move_table_selection,
        },
        components::{
            format::{format_brl, parse_positive_amount},
            input::InputField,
            popup::ConfirmPopup,
            toggle::{push_form_error, render_selector, render_toggle},
        },
    },
};

pub enum BudgetFormMode {
    Create,
    Edit(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetField {
    Category,
    Amount,
    Period,
}

impl BudgetField {
    pub const ALL: [Self; 3] = [Self::Category, Self::Amount, Self::Period];
}

pub struct BudgetForm {
    pub mode: BudgetFormMode,
    pub category_idx: usize,
    pub amount: InputField,
    pub period_idx: usize,
    pub active_field: usize,
    pub error: Option<String>,
}

pub struct ValidatedBudget {
    pub category_id: i32,
    pub amount: Decimal,
    pub period: BudgetPeriod,
}

pub const PERIODS: [BudgetPeriod; 3] = [
    BudgetPeriod::Weekly,
    BudgetPeriod::Monthly,
    BudgetPeriod::Yearly,
];

impl BudgetForm {
    pub fn validate(&self, categories: &[Category]) -> Result<ValidatedBudget, String> {
        let amount = parse_positive_amount(&self.amount.value)?;

        let expense_categories: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parsed_type() == CategoryType::Expense)
            .collect();
        let category_id = expense_categories
            .get(self.category_idx)
            .map(|c| c.id)
            .ok_or("No category selected")?;

        let period = PERIODS
            .get(self.period_idx)
            .copied()
            .ok_or("No period selected")?;

        Ok(ValidatedBudget {
            category_id,
            amount,
            period,
        })
    }

    pub fn new_create() -> Self {
        Self {
            mode: BudgetFormMode::Create,
            category_idx: 0,
            amount: InputField::new("Amount"),
            period_idx: 1, // default to Monthly
            active_field: 0,
            error: None,
        }
    }

    pub fn new_edit(budget: &crate::models::Budget, categories: &[Category]) -> Self {
        let expense_categories: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parsed_type() == CategoryType::Expense)
            .collect();
        let category_idx = expense_categories
            .iter()
            .position(|c| c.id == budget.category_id)
            .unwrap_or(0);

        let period_idx = PERIODS
            .iter()
            .position(|p| p.as_str() == budget.period)
            .unwrap_or(1);

        Self {
            mode: BudgetFormMode::Edit(budget.id),
            category_idx,
            amount: InputField::new("Amount").with_value(budget.amount.to_string()),
            period_idx,
            active_field: 1, // start on Amount (the only editable field)
            error: None,
        }
    }

    pub fn active_field_id(&self) -> BudgetField {
        BudgetField::ALL[self.active_field.min(BudgetField::ALL.len() - 1)]
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.budget_form.is_some() {
        render_form(frame, area, app);
    } else {
        render_list(frame, area, app);
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let [table_area, detail_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);

    let header = Row::new(["Category", "Amount", "Period", "Spent", "%"])
        .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .budgets
        .iter()
        .map(|b| {
            let category_name = app.category_name(b.category_id);

            let spent = app.budget_spent.get(&b.id).copied().unwrap_or(Decimal::ZERO);
            let pct = if b.amount > Decimal::ZERO {
                (spent * Decimal::from(100)) / b.amount
            } else {
                Decimal::ZERO
            };

            let pct_style = if pct > Decimal::from(100) {
                Style::new().fg(Color::Red)
            } else if pct > Decimal::from(80) {
                Style::new().fg(Color::Yellow)
            } else {
                Style::new().fg(Color::Green)
            };

            Row::new([
                category_name.to_string(),
                format_brl(b.amount),
                b.parsed_period().label().to_string(),
                format_brl(spent),
                format!("{}%", pct.round()),
            ])
            .style(pct_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(12),
            Constraint::Length(14),
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Length(6),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Budgets"))
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.budget_table_state);

    let detail_content = match app
        .budget_table_state
        .selected()
        .and_then(|i| app.budgets.get(i))
    {
        Some(b) => {
            let category_name = app.category_name(b.category_id);

            let spent = app.budget_spent.get(&b.id).copied().unwrap_or(Decimal::ZERO);

            vec![
                Line::from(format!(
                    " {} | {} | {}",
                    category_name,
                    b.parsed_period().label(),
                    format_brl(b.amount),
                )),
                Line::from(format!(" Spent: {}", format_brl(spent))),
                Line::from(format!(
                    " Created: {}",
                    b.created_at.format("%d-%m-%Y %H:%M")
                )),
                Line::from(Span::styled(
                    " [n] New  [e] Edit  [d] Delete  [x] Export",
                    Style::new().fg(Color::DarkGray),
                )),
            ]
        }
        None => vec![Line::from(" No budget selected.")],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("Budget Details");
    let detail = Paragraph::new(detail_content).block(detail_block);
    frame.render_widget(detail, detail_area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.budget_form.as_ref().unwrap();
    let is_edit = matches!(form.mode, BudgetFormMode::Edit(_));

    let title = if is_edit { "Edit Budget" } else { "New Budget" };

    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in BudgetField::ALL.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            BudgetField::Category => {
                let names: Vec<&str> = app
                    .categories
                    .iter()
                    .filter(|c| c.parsed_type() == CategoryType::Expense)
                    .map(|c| c.name.as_str())
                    .collect();
                if is_edit {
                    Line::from(format!(
                        " Category: {} (locked)",
                        names.get(form.category_idx).unwrap_or(&"?")
                    ))
                } else {
                    render_selector("Category", &names, form.category_idx, active, "no expense categories")
                }
            }
            BudgetField::Amount => form.amount.render_line(active),
            BudgetField::Period => {
                let labels: Vec<&str> = PERIODS.iter().map(|p| p.label()).collect();
                if is_edit {
                    Line::from(format!(
                        " Period: {} (locked)",
                        labels.get(form.period_idx).unwrap_or(&"?")
                    ))
                } else {
                    render_toggle("Period", &labels, form.period_idx, active)
                }
            }
        };
        lines.push(line);
    }

    push_form_error(&mut lines, &form.error);

    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

// ── Key handling & form submission (impl App) ─────────────────────

impl App {
    pub(crate) async fn handle_budgets_key(&mut self, code: KeyCode) -> anyhow::Result<()> {
        match code {
            KeyCode::Up => {
                move_table_selection(&mut self.budget_table_state, self.budgets.len(), -1);
            }
            KeyCode::Down => {
                move_table_selection(&mut self.budget_table_state, self.budgets.len(), 1);
            }
            KeyCode::Char('n') => {
                let has_expense_cat = self
                    .categories
                    .iter()
                    .any(|c| c.parsed_type() == CategoryType::Expense);
                if !has_expense_cat {
                    self.status_message =
                        Some(StatusMessage::error("Create an expense category first"));
                } else {
                    self.budget_form = Some(BudgetForm::new_create());
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('e') => {
                if let Some(b) = self
                    .budget_table_state
                    .selected()
                    .and_then(|i| self.budgets.get(i))
                {
                    let b = b.clone();
                    self.budget_form = Some(BudgetForm::new_edit(&b, &self.categories));
                    self.input_mode = InputMode::Editing;
                }
            }
            KeyCode::Char('d') => {
                if let Some(b) = self
                    .budget_table_state
                    .selected()
                    .and_then(|i| self.budgets.get(i))
                {
                    let budget_id = b.id;
                    let category_name = self.category_name(b.category_id).to_string();
                    self.confirm_action = Some(ConfirmAction::DeleteBudget(budget_id));
                    self.confirm_popup = Some(ConfirmPopup::new(format!(
                        "Delete budget for \"{category_name}\"?"
                    )));
                }
            }
            KeyCode::Char('x') => {
                let cats = &self.categories;
                match crate::export::export_budgets(
                    &self.budgets,
                    |id| {
                        cats.iter()
                            .find(|c| c.id == id)
                            .map(|c| c.name.clone())
                            .unwrap_or_else(|| "?".into())
                    },
                    &self.budget_spent,
                ) {
                    Ok(path) => {
                        self.status_message = Some(StatusMessage::info(format!(
                            "Exported {} budgets to {}",
                            self.budgets.len(),
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

    pub(crate) fn handle_budget_form_key(&mut self, code: KeyCode) {
        let form = self.budget_form.as_mut().unwrap();
        let is_edit = matches!(form.mode, BudgetFormMode::Edit(_));

        match code {
            KeyCode::Tab | KeyCode::Down => {
                if is_edit {
                    // Only Amount (index 1) is editable in edit mode
                    form.active_field = 1;
                } else if form.active_field < BudgetField::ALL.len() - 1 {
                    form.active_field += 1;
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if is_edit {
                    form.active_field = 1;
                } else if form.active_field > 0 {
                    form.active_field -= 1;
                }
            }
            _ => match form.active_field_id() {
                BudgetField::Amount => form.amount.handle_key(code),
                BudgetField::Category if !is_edit => {
                    if is_toggle_key(code) {
                        let len = self
                            .categories
                            .iter()
                            .filter(|c| c.parsed_type() == CategoryType::Expense)
                            .count();
                        cycle_index(&mut form.category_idx, len, code);
                    }
                }
                BudgetField::Period if !is_edit => {
                    if is_toggle_key(code) {
                        cycle_index(&mut form.period_idx, PERIODS.len(), code);
                    }
                }
                _ => {} // locked fields in edit mode
            },
        }
    }

    pub(crate) async fn submit_budget_form(&mut self) -> anyhow::Result<()> {
        use crate::db::budgets;

        let form = self.budget_form.as_ref().unwrap();
        let validated = match form.validate(&self.categories) {
            Ok(v) => v,
            Err(e) => {
                self.budget_form.as_mut().unwrap().error = Some(e);
                return Ok(());
            }
        };

        let is_edit = match form.mode {
            BudgetFormMode::Create => false,
            BudgetFormMode::Edit(_) => true,
        };

        if is_edit {
            let id = match form.mode {
                BudgetFormMode::Edit(id) => id,
                _ => unreachable!(),
            };
            budgets::update_budget(&self.pool, id, validated.amount).await?;
            info!(id, amount = %validated.amount, "budget updated");
        } else {
            match budgets::create_budget(
                &self.pool,
                validated.category_id,
                validated.amount,
                validated.period,
            )
            .await
            {
                Ok(_) => {
                    info!(
                        category_id = validated.category_id,
                        amount = %validated.amount,
                        "budget created"
                    );
                }
                Err(e) => {
                    if e.as_database_error()
                        .is_some_and(|db_err| db_err.is_unique_violation())
                    {
                        self.budget_form.as_mut().unwrap().error =
                            Some("A budget for this category and period already exists".into());
                        return Ok(());
                    }
                    return Err(e.into());
                }
            }
        }

        self.budget_form = None;
        self.input_mode = InputMode::Normal;
        self.load_data().await?;
        Ok(())
    }

    pub(crate) async fn execute_delete_budget(&mut self, id: i32) -> anyhow::Result<()> {
        crate::db::budgets::delete_budget(&self.pool, id).await?;
        info!(id, "budget deleted");
        self.load_data().await?;
        clamp_selection(&mut self.budget_table_state, self.budgets.len());
        Ok(())
    }
}
