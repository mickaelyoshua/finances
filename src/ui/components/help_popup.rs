use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::ui::{
    app::Screen,
    i18n::Locale,
};

pub struct HelpPopup {
    pub screen: Screen,
}

impl HelpPopup {
    pub fn new(screen: Screen) -> Self {
        Self { screen }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        matches!(code, KeyCode::Esc | KeyCode::Char('?'))
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, locale: Locale) {
        let (desc, keys) = screen_help(self.screen, locale);

        // height = 1 (desc) + 1 (blank) + keys.len() + 1 (blank) + 1 (dismiss) + 2 (border)
        let height = (keys.len() as u16) + 6;
        let width = 60u16.min(area.width.saturating_sub(4));
        let popup_area = centered_rect(width, height, area);

        let mut lines = vec![
            Line::from(Span::styled(
                format!(" {desc}"),
                Style::new().fg(Color::White),
            )),
            Line::from(""),
        ];

        for (key, action) in &keys {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<12}"), Style::new().fg(Color::Yellow)),
                Span::raw(*action),
            ]));
        }

        lines.push(Line::from(""));
        let dismiss = match locale {
            Locale::En => "Press ? or Esc to close",
            Locale::Pt => "Pressione ? ou Esc para fechar",
        };
        lines.push(Line::from(Span::styled(
            format!(" {dismiss}"),
            Style::new().fg(Color::DarkGray),
        )));

        let title = match locale {
            Locale::En => "Help",
            Locale::Pt => "Ajuda",
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::new().fg(Color::Cyan));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(Paragraph::new(lines).block(block), popup_area);
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

/// (screen description, vec of (key, action) pairs)
type HelpContent = (&'static str, Vec<(&'static str, &'static str)>);

fn screen_help(screen: Screen, locale: Locale) -> HelpContent {
    match (screen, locale) {
        // ── Dashboard ──
        (Screen::Dashboard, Locale::En) => (
            "Overview of balances, budgets, pending items, and notifications.",
            vec![
                ("j / k", "Navigate notifications"),
                ("r", "Dismiss selected notification"),
                ("R", "Dismiss all notifications"),
                ("Ctrl+L", "Toggle language"),
                ("?", "Show this help"),
            ],
        ),
        (Screen::Dashboard, Locale::Pt) => (
            "Visão geral de saldos, orçamentos, pendências e notificações.",
            vec![
                ("j / k", "Navegar notificações"),
                ("r", "Dispensar notificação selecionada"),
                ("R", "Dispensar todas as notificações"),
                ("Ctrl+L", "Alternar idioma"),
                ("?", "Mostrar esta ajuda"),
            ],
        ),

        // ── Transactions ──
        (Screen::Transactions, Locale::En) => (
            "View, create, edit, and delete financial transactions.",
            vec![
                ("n", "New transaction"),
                ("e", "Edit selected"),
                ("d", "Delete selected"),
                ("f", "Open filter bar"),
                ("r", "Reset filters"),
                ("x", "Export to CSV"),
                ("PgUp/PgDn", "Previous/next page"),
                ("Ctrl+L", "Toggle language"),
                ("?", "Show this help"),
            ],
        ),
        (Screen::Transactions, Locale::Pt) => (
            "Visualize, crie, edite e exclua transações financeiras.",
            vec![
                ("n", "Nova transação"),
                ("e", "Editar selecionada"),
                ("d", "Excluir selecionada"),
                ("f", "Abrir barra de filtros"),
                ("r", "Limpar filtros"),
                ("x", "Exportar para CSV"),
                ("PgUp/PgDn", "Página anterior/próxima"),
                ("Ctrl+L", "Alternar idioma"),
                ("?", "Mostrar esta ajuda"),
            ],
        ),

        // ── Accounts ──
        (Screen::Accounts, Locale::En) => (
            "Manage bank accounts and wallets. Balances are computed automatically.",
            vec![
                ("n", "New account"),
                ("e", "Edit selected"),
                ("d", "Deactivate selected"),
                ("x", "Export to CSV"),
                ("Ctrl+L", "Toggle language"),
                ("?", "Show this help"),
            ],
        ),
        (Screen::Accounts, Locale::Pt) => (
            "Gerencie contas bancárias e carteiras. Saldos são calculados automaticamente.",
            vec![
                ("n", "Nova conta"),
                ("e", "Editar selecionada"),
                ("d", "Desativar selecionada"),
                ("x", "Exportar para CSV"),
                ("Ctrl+L", "Alternar idioma"),
                ("?", "Mostrar esta ajuda"),
            ],
        ),

        // ── Budgets ──
        (Screen::Budgets, Locale::En) => (
            "Set spending limits per category with weekly, monthly, or yearly periods.",
            vec![
                ("n", "New budget"),
                ("e", "Edit selected"),
                ("d", "Delete selected"),
                ("x", "Export to CSV"),
                ("Ctrl+L", "Toggle language"),
                ("?", "Show this help"),
            ],
        ),
        (Screen::Budgets, Locale::Pt) => (
            "Defina limites de gastos por categoria com períodos semanal, mensal ou anual.",
            vec![
                ("n", "Novo orçamento"),
                ("e", "Editar selecionado"),
                ("d", "Excluir selecionado"),
                ("x", "Exportar para CSV"),
                ("Ctrl+L", "Alternar idioma"),
                ("?", "Mostrar esta ajuda"),
            ],
        ),

        // ── Categories ──
        (Screen::Categories, Locale::En) => (
            "Organize transactions by expense and income categories.",
            vec![
                ("n", "New category"),
                ("e", "Edit selected"),
                ("d", "Delete selected"),
                ("x", "Export to CSV"),
                ("Ctrl+L", "Toggle language"),
                ("?", "Show this help"),
            ],
        ),
        (Screen::Categories, Locale::Pt) => (
            "Organize transações por categorias de despesa e receita.",
            vec![
                ("n", "Nova categoria"),
                ("e", "Editar selecionada"),
                ("d", "Excluir selecionada"),
                ("x", "Exportar para CSV"),
                ("Ctrl+L", "Alternar idioma"),
                ("?", "Mostrar esta ajuda"),
            ],
        ),

        // ── Recurring ──
        (Screen::Recurring, Locale::En) => (
            "Manage recurring transactions. Confirm pending items when they occur.",
            vec![
                ("c", "Confirm pending transaction"),
                ("n", "New recurring transaction"),
                ("e", "Edit selected"),
                ("d", "Deactivate selected"),
                ("x", "Export to CSV"),
                ("Ctrl+L", "Toggle language"),
                ("?", "Show this help"),
            ],
        ),
        (Screen::Recurring, Locale::Pt) => (
            "Gerencie transações recorrentes. Confirme pendentes quando ocorrerem.",
            vec![
                ("c", "Confirmar transação pendente"),
                ("n", "Nova transação recorrente"),
                ("e", "Editar selecionada"),
                ("d", "Desativar selecionada"),
                ("x", "Exportar para CSV"),
                ("Ctrl+L", "Alternar idioma"),
                ("?", "Mostrar esta ajuda"),
            ],
        ),

        // ── Transfers ──
        (Screen::Transfers, Locale::En) => (
            "Transfer money between different accounts.",
            vec![
                ("n", "New transfer"),
                ("d", "Delete selected"),
                ("x", "Export to CSV"),
                ("PgUp/PgDn", "Previous/next page"),
                ("Ctrl+L", "Toggle language"),
                ("?", "Show this help"),
            ],
        ),
        (Screen::Transfers, Locale::Pt) => (
            "Transfira dinheiro entre contas diferentes.",
            vec![
                ("n", "Nova transferência"),
                ("d", "Excluir selecionada"),
                ("x", "Exportar para CSV"),
                ("PgUp/PgDn", "Página anterior/próxima"),
                ("Ctrl+L", "Alternar idioma"),
                ("?", "Mostrar esta ajuda"),
            ],
        ),

        // ── CC Payments ──
        (Screen::CreditCardPayments, Locale::En) => (
            "Record credit card bill payments within the same account.",
            vec![
                ("n", "New payment"),
                ("d", "Delete selected"),
                ("x", "Export to CSV"),
                ("PgUp/PgDn", "Previous/next page"),
                ("Ctrl+L", "Toggle language"),
                ("?", "Show this help"),
            ],
        ),
        (Screen::CreditCardPayments, Locale::Pt) => (
            "Registre pagamentos de fatura de cartão dentro da mesma conta.",
            vec![
                ("n", "Novo pagamento"),
                ("d", "Excluir selecionado"),
                ("x", "Exportar para CSV"),
                ("PgUp/PgDn", "Página anterior/próxima"),
                ("Ctrl+L", "Alternar idioma"),
                ("?", "Mostrar esta ajuda"),
            ],
        ),

        // ── CC Statements ──
        (Screen::CreditCardStatements, Locale::En) => (
            "View credit card statement periods, charges, and payment status.",
            vec![
                ("Enter", "View statement transactions"),
                ("p", "Pay selected statement"),
                ("u", "Unpay (remove payments)"),
                ("h / l", "Switch credit card account"),
                ("Esc", "Back to statement list"),
                ("Ctrl+L", "Toggle language"),
                ("?", "Show this help"),
            ],
        ),
        (Screen::CreditCardStatements, Locale::Pt) => (
            "Veja períodos de fatura do cartão, cobranças e status de pagamento.",
            vec![
                ("Enter", "Ver transações da fatura"),
                ("p", "Pagar fatura selecionada"),
                ("u", "Estornar (remover pagamentos)"),
                ("h / l", "Trocar conta do cartão"),
                ("Esc", "Voltar à lista de faturas"),
                ("Ctrl+L", "Alternar idioma"),
                ("?", "Mostrar esta ajuda"),
            ],
        ),
    }
}
