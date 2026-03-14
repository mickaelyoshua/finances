use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Row, Table},
};
use rust_decimal::Decimal;

use crate::{
    models::{Account, AccountType},
    ui::{
        App,
        components::{
            format::format_brl,
            input::InputField,
            toggle::{push_form_error, render_toggle},
        },
    },
};

pub enum AccountFormMode {
    Create,
    Edit(i32),
}

pub struct AccountForm {
    pub mode: AccountFormMode,
    pub name: InputField,
    pub account_type: AccountType,
    pub has_credit_card: bool,
    pub credit_limit: InputField,
    pub billing_day: InputField,
    pub due_day: InputField,
    pub has_debit_card: bool,
    pub active_field: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountField {
    Name,
    AccountType,
    HasCreditCard,
    CreditLimit,
    BillingDay,
    DueDay,
    HasDebitCard,
}

pub struct ValidatedAccount {
    pub name: String,
    pub account_type: AccountType,
    pub has_credit_card: bool,
    pub credit_limit: Option<Decimal>,
    pub billing_day: Option<i16>,
    pub due_day: Option<i16>,
    pub has_debit_card: bool,
}

impl AccountForm {
    pub fn validate(&self) -> Result<ValidatedAccount, String> {
        let name = self.name.value.trim().to_string();
        if name.is_empty() {
            return Err("Name is required".into());
        }

        let credit_limit = if self.has_credit_card {
            match self.credit_limit.value.trim().parse::<Decimal>() {
                Ok(v) if v > Decimal::ZERO => Some(v),
                Ok(_) => return Err("Credit limit must be positive".into()),
                Err(_) => return Err("Invalid credit limit".into()),
            }
        } else {
            None
        };

        let billing_day = if self.has_credit_card {
            match self.billing_day.value.trim().parse::<i16>() {
                Ok(v) if (1..=28).contains(&v) => Some(v),
                _ => return Err("Billing day must be 1-28".into()),
            }
        } else {
            None
        };

        let due_day = if self.has_credit_card {
            match self.due_day.value.trim().parse::<i16>() {
                Ok(v) if (1..=28).contains(&v) => Some(v),
                _ => return Err("Due day must be 1-28".into()),
            }
        } else {
            None
        };

        Ok(ValidatedAccount {
            name,
            account_type: self.account_type,
            has_credit_card: self.has_credit_card,
            credit_limit,
            billing_day,
            due_day,
            has_debit_card: self.has_debit_card,
        })
    }

    pub fn new_create() -> Self {
        Self {
            mode: AccountFormMode::Create,
            name: InputField::new("Name"),
            account_type: AccountType::Checking,
            has_credit_card: false,
            credit_limit: InputField::new("Credit Limit"),
            billing_day: InputField::new("Billing Day"),
            due_day: InputField::new("Due Day"),
            has_debit_card: false,
            active_field: 0,
            error: None,
        }
    }

    pub fn new_edit(acc: &Account) -> Self {
        Self {
            mode: AccountFormMode::Edit(acc.id),
            name: InputField::new("Name").with_value(&acc.name),
            account_type: acc.parsed_type(),
            has_credit_card: acc.has_credit_card,
            credit_limit: InputField::new("Credit Limit")
                .with_value(acc.credit_limit.map(|v| v.to_string()).unwrap_or_default()),
            billing_day: InputField::new("Billing Day")
                .with_value(acc.billing_day.map(|v| v.to_string()).unwrap_or_default()),
            due_day: InputField::new("Due Day")
                .with_value(acc.due_day.map(|v| v.to_string()).unwrap_or_default()),
            has_debit_card: acc.has_debit_card,
            active_field: 0,
            error: None,
        }
    }

    pub fn visible_fields(&self) -> Vec<AccountField> {
        let mut fields = vec![AccountField::Name, AccountField::AccountType];

        if self.account_type == AccountType::Checking {
            fields.push(AccountField::HasCreditCard);
            if self.has_credit_card {
                fields.push(AccountField::CreditLimit);
                fields.push(AccountField::BillingDay);
                fields.push(AccountField::DueDay);
            }
            fields.push(AccountField::HasDebitCard);
        }
        fields
    }

    pub fn active_field_id(&self) -> AccountField {
        let visible = self.visible_fields();
        visible[self.active_field.min(visible.len() - 1)]
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.account_form.is_some() {
        render_form(frame, area, app);
    } else {
        render_list(frame, area, app);
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let header = Row::new([
        "Account",
        "Type",
        "Credit Card",
        "Debit",
        "Checking",
        "Credit Used",
    ])
    .style(Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    let [table_area, detail_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);

    let rows: Vec<Row> = app
        .accounts
        .iter()
        .map(|acc| {
            let (checking, credit) = app.balances.get(&acc.id).copied().unwrap_or_default();
            let credit_cell = if acc.has_credit_card {
                let limit = acc.credit_limit.unwrap_or(Decimal::ZERO);
                format!("{} / {}", format_brl(credit), format_brl(limit))
            } else {
                "-".to_string()
            };
            Row::new([
                acc.name.clone(),
                acc.parsed_type().label().to_string(),
                if acc.has_credit_card { "Yes" } else { "No" }.to_string(),
                if acc.has_debit_card { "Yes" } else { "No" }.to_string(),
                format_brl(checking),
                credit_cell,
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(12),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Length(14),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Accounts"))
    .row_highlight_style(
        Style::new()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    frame.render_stateful_widget(table, table_area, &mut app.account_table_state);

    let detail_content = match app
        .account_table_state
        .selected()
        .and_then(|i| app.accounts.get(i))
    {
        Some(acc) => {
            let methods: Vec<&str> = acc
                .allowed_payment_methods()
                .iter()
                .map(|m| m.label())
                .collect();

            let billing = if acc.has_credit_card {
                format!(
                    "Billing day: {} | Due day: {}",
                    acc.billing_day.unwrap_or(0),
                    acc.due_day.unwrap_or(0),
                )
            } else {
                "No credit card".to_string()
            };

            vec![
                Line::from(format!(
                    " Name: {} | Type: {}",
                    acc.name,
                    acc.parsed_type().label()
                )),
                Line::from(format!(" {}", billing)),
                Line::from(format!(" Payment methods: {}", methods.join(", "))),
                Line::from(format!(" Created: {}", acc.created_at.format("%d-%m-%Y"))),
            ]
        }
        None => vec![Line::from(" No account selected.")],
    };

    let detail_block = Block::default()
        .borders(Borders::ALL)
        .title("Account Details");
    let detail = Paragraph::new(detail_content).block(detail_block);

    frame.render_widget(detail, detail_area);
}

fn render_form(frame: &mut Frame, area: Rect, app: &mut App) {
    let form = app.account_form.as_ref().unwrap();
    let visible = form.visible_fields();

    let title = match form.mode {
        AccountFormMode::Create => "New Account",
        AccountFormMode::Edit(_) => "Edit Account",
    };

    let mut lines: Vec<Line> = Vec::new();

    for (i, field) in visible.iter().enumerate() {
        let active = i == form.active_field;
        let line = match field {
            AccountField::Name => form.name.render_line(active),
            AccountField::CreditLimit => form.credit_limit.render_line(active),
            AccountField::BillingDay => form.billing_day.render_line(active),
            AccountField::DueDay => form.due_day.render_line(active),
            AccountField::AccountType => render_toggle(
                "Type",
                &["Checking", "Cash"],
                if form.account_type == AccountType::Checking {
                    0
                } else {
                    1
                },
                active,
            ),
            AccountField::HasCreditCard => render_toggle(
                "Credit Card",
                &["No", "Yes"],
                if form.has_credit_card { 1 } else { 0 },
                active,
            ),
            AccountField::HasDebitCard => render_toggle(
                "Debit Card",
                &["No", "Yes"],
                if form.has_debit_card { 1 } else { 0 },
                active,
            ),
        };
        lines.push(line);
    }

    push_form_error(&mut lines, &form.error);

    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
