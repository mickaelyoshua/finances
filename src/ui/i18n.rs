/// Locale and translation infrastructure.
///
/// Two languages: English (default) and Brazilian Portuguese.
/// `t(locale, key)` returns a static translation; unknown keys
/// fall back to the key itself. `Locale::enum_label()` bridges
/// existing `.label()` methods without touching their signatures.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    En,
    Pt,
}

impl Locale {
    pub fn toggle(self) -> Self {
        match self {
            Self::En => Self::Pt,
            Self::Pt => Self::En,
        }
    }

    /// Translate an enum's English `.label()` to the active locale.
    /// Exists because enum `Display`/`FromStr` impls double as DB values —
    /// changing them would break serialization. This translates at the UI layer.
    pub fn enum_label(self, en_label: &'static str) -> &'static str {
        if self == Locale::En {
            return en_label;
        }
        match en_label {
            "Expense" => "Despesa",
            "Income" => "Receita",
            "Checking" => "Corrente",
            "Cash" => "Dinheiro",
            "PIX" => "PIX",
            "Credit Card" => "Cartão de Crédito",
            "Debit Card" => "Cartão de Débito",
            "Boleto" => "Boleto",
            "Transfer" => "Transferência",
            "Daily" => "Diário",
            "Weekly" => "Semanal",
            "Monthly" => "Mensal",
            "Yearly" => "Anual",
            "Upcoming" => "Futura",
            "Open" => "Aberta",
            "Paid" => "Paga",
            "Due" => "Pendente",
            other => other,
        }
    }
}

/// Return the translated string for `key` in the given `locale`.
/// PT returns `Option` (missing translation falls through); EN is the
/// identity fallback — unknown keys return the key itself.
pub fn t(locale: Locale, key: &'static str) -> &'static str {
    if locale == Locale::Pt
        && let Some(s) = t_pt(key)
    {
        return s;
    }
    t_en(key)
}

fn t_en(key: &'static str) -> &'static str {
    match key {
        // ── Screen labels ──
        "screen.dashboard" => "Dashboard",
        "screen.transactions" => "Transactions",
        "screen.accounts" => "Accounts",
        "screen.budgets" => "Budgets",
        "screen.categories" => "Categories",
        "screen.installments" => "Installments",
        "screen.recurring" => "Recurring",
        "screen.transfers" => "Transfers",
        "screen.cc_payments" => "CC Payments",
        "screen.cc_statements" => "CC Statements",
        "screen.reports" => "Reports",

        // ── Form labels ──
        "form.name" => "Name",
        "form.name_pt" => "Portuguese Name",
        "form.amount" => "Amount",
        "form.date" => "Date",
        "form.description" => "Description",
        "form.billing_day" => "Billing Day",
        "form.due_day" => "Due Day",
        "form.credit_limit" => "Credit Limit",
        "form.from" => "From",
        "form.to" => "To",
        "form.desc" => "Desc",
        "form.purchase_date" => "Purchase Date",
        "form.installments" => "Installments",
        "form.next_due" => "Next Due",
        "form.total_amount" => "Total Amount",
        "form.is_installment" => "Installment",

        // ── Form selector/toggle labels ──
        "form.type" => "Type",
        "form.account" => "Account",
        "form.payment" => "Payment",
        "form.category" => "Category",
        "form.period" => "Period",
        "form.frequency" => "Frequency",
        "form.credit_card" => "Credit Card",
        "form.debit_card" => "Debit Card",

        // ── Filter labels ──
        "filter.acct" => "Acct",
        "filter.cat" => "Cat",
        "filter.type" => "Type",
        "filter.pay" => "Pay",
        "filter.all" => "All",

        // ── Table headers ──
        "header.account" => "Account",
        "header.type" => "Type",
        "header.credit_card" => "Credit Card",
        "header.debit" => "Debit",
        "header.checking" => "Checking",
        "header.credit_used" => "Credit Used",
        "header.date" => "Date",
        "header.description" => "Description",
        "header.amount" => "Amount",
        "header.category" => "Category",
        "header.period" => "Period",
        "header.spent" => "Spent",
        "header.pct" => "%",
        "header.total" => "Total",
        "header.num_inst" => "# Inst.",
        "header.purchase_date" => "Purchase Date",
        "header.frequency" => "Frequency",
        "header.next_due" => "Next Due",
        "header.from" => "From",
        "header.to" => "To",
        "header.balance" => "Balance",
        "header.status" => "Status",
        "header.filters" => "Filters",

        // ── Block/section titles ──
        "title.accounts" => "Accounts",
        "title.account_details" => "Account Details",
        "title.new_account" => "New Account",
        "title.edit_account" => "Edit Account",
        "title.transactions" => "Transactions",
        "title.transaction_details" => "Transaction Details",
        "title.new_transaction" => "New Transaction",
        "title.edit_transaction" => "Edit Transaction",
        "title.categories" => "Categories",
        "title.category_details" => "Category Details",
        "title.new_category" => "New Category",
        "title.edit_category" => "Edit Category",
        "title.budgets" => "Budgets",
        "title.budget_details" => "Budget Details",
        "title.new_budget" => "New Budget",
        "title.edit_budget" => "Edit Budget",
        "title.installments" => "Installment Purchases",
        "title.installment_details" => "Installment Details",
        "title.new_installment" => "New Installment Purchase",
        "title.edit_installment" => "Edit Installment Purchase",
        "title.confirm_installment" => "Confirm Installment",
        "title.confirm_installment_edit" => "Confirm Installment Edit",
        "title.recurring" => "Recurring Transactions",
        "title.recurring_details" => "Recurring Details",
        "title.new_recurring" => "New Recurring Transaction",
        "title.edit_recurring" => "Edit Recurring Transaction",
        "title.transfers" => "Transfers",
        "title.transfer_details" => "Transfer Details",
        "title.new_transfer" => "New Transfer",
        "title.cc_payments" => "Credit Card Payments",
        "title.payment_details" => "Payment Details",
        "title.new_cc_payment" => "New Credit Card Payment",
        "title.cc_statements" => "CC Statements",
        "title.statement_details" => "Statement Details",
        "title.statement" => "Statement",
        "title.details" => "Details",
        "title.accounts_balances" => "Accounts Balances",
        "title.budget_status" => "Budget Status",
        "title.pending_recurring" => "Pending Recurring",
        "title.current_cc_statements" => "Current CC Statements (Open)",
        "title.confirm" => "Confirm",
        "title.filter" => "Filter (Enter=apply, Esc=close, r=reset)",
        "title.reports" => "Reports",
        "title.expenses_by_category" => "Expenses by Category",
        "title.income_by_category" => "Income by Category",
        "title.monthly_trend" => "Monthly Trend",
        "title.report_filters" => "Report Filters (Enter=apply, Esc=close)",

        // ── Reports ──
        "report.no_data" => "No data for this period.",
        "report.filter.all_accounts" => "All accounts",
        "report.filter.all_methods" => "All payment methods",
        "report.preset.this_month" => "This month",
        "report.preset.last_month" => "Last month",
        "report.preset.last_3_months" => "Last 3 months",
        "report.preset.ytd" => "Year to date",
        "report.preset.this_year" => "This year",
        "report.preset.last_year" => "Last year",
        "report.preset.custom" => "Custom",

        // ── Status bar keybinding text ──
        "status.quit" => "Quit",
        "status.screen" => "Screen",
        "status.navigate" => "Navigate",
        "status.cancel" => "Cancel",
        "status.nav_fields" => "Navigate",
        "status.toggle" => "Toggle",
        "status.submit" => "Submit",
        "status.close" => "Close",
        "status.apply" => "Apply",
        "status.cycle" => "Cycle",
        "status.unread" => "unread",
        "status.export_report" => "Report exported",

        // ── Detail pane keybinding hints ──
        "hint.acct" => "[n] New  [e] Edit  [d] Deactivate  [x] Export",
        "hint.cat" => "[n] New  [e] Edit  [d] Delete  [x] Export",
        "hint.txn" => "[n] New  [e] Edit  [d] Delete  [f] Filter  [r] Reset  [x] Export",
        "hint.budget" => "[n] New  [e] Edit  [d] Delete  [x] Export",
        "hint.inst" => "[n] New  [e] Edit  [d] Delete  [x] Export",
        "hint.recurring" => "[c] Confirm pending  [n] New  [e] Edit  [d] Deactivate  [x] Export",
        "hint.xfer" => "[n] New  [d] Delete  [x] Export",
        "hint.cc_pay" => "[n] New  [d] Delete  [x] Export",
        "hint.cc_stmt_list" => "[Enter] View transactions  [p] Pay  [u] Unpay  [h/l] Switch account",
        "hint.cc_stmt_list_all" => "[h/l] Switch account",
        "hint.cc_stmt_detail" => "[Esc] Back  [Enter] Go to transaction / installment  [j/k] Navigate",
        "hint.reports" => "[Tab] Cycle view  [p] Cycle preset  [f] Filter  [j/k] Scroll  [x] Export HTML",

        // ── Status messages ──
        "msg.create_account_first" => "Create an account first",
        "msg.create_category_first" => "Create a category first",
        "msg.create_expense_cat_first" => "Create an expense category first",
        "msg.no_cc_account" => "No account with credit card available",
        "msg.need_two_accounts" => "Need at least 2 accounts for a transfer",
        "msg.installment_edit_hint" => "Press [e] to edit or [d] to delete the installment group",
        "msg.select_account_view" => "Select a specific account to view transactions",
        "msg.select_account_pay" => "Select a specific account to pay a statement",
        "msg.select_account_unpay" => "Select a specific account to unpay a statement",
        "msg.cannot_pay_upcoming" => "Cannot pay an upcoming statement",
        "msg.cannot_pay_open" => "Cannot pay an open statement",
        "msg.already_paid" => "Statement already fully paid",
        "msg.only_closed_unpay" => "Only closed statements can be unpaid",
        "msg.no_payments" => "Statement has no payments",
        "msg.statement_paid" => "Statement paid",

        // ── Validation errors ──
        "err.name_required" => "Name is required",
        "err.description_required" => "Description is required",
        "err.invalid_date" => "Invalid date (use DD-MM-YYYY)",
        "err.invalid_amount" => "Invalid amount",
        "err.amount_positive" => "Amount must be positive",
        "err.credit_limit_positive" => "Credit limit must be positive",
        "err.invalid_credit_limit" => "Invalid credit limit",
        "err.billing_day_range" => "Billing day must be 1-31",
        "err.due_day_range" => "Due day must be 1-31",
        "err.no_account" => "No account selected",
        "err.no_source_account" => "No source account selected",
        "err.no_dest_account" => "No destination account selected",
        "err.same_account" => "Source and destination must be different accounts",
        "err.no_payment_method" => "No payment method selected",
        "err.no_category" => "No category selected",
        "err.no_period" => "No period selected",
        "err.no_frequency" => "No frequency selected",
        "err.at_least_two_inst" => "Must be at least 2 installments",
        "err.invalid_inst_count" => "Invalid installment count",
        "err.budget_exists" => "A budget for this category and period already exists",
        "err.cannot_change_acct_type" => "Cannot change account type: account has transactions",
        "err.cannot_disable_credit" => "Cannot disable credit card: account has credit transactions",
        "err.cannot_disable_debit" => "Cannot disable debit card: account has debit transactions",
        "err.cannot_change_cat_type" => "Cannot change type: category is referenced by transactions or budgets",

        // ── CC Statement status ──
        "stmt.upcoming" => "Upcoming",
        "stmt.open" => "Open",
        "stmt.paid" => "Paid",
        "stmt.due" => "Due",
        "stmt.all_accounts" => "All Accounts",

        // ── Misc ──
        "misc.yes" => "Yes",
        "misc.no" => "No",
        "misc.all" => "All",
        "misc.more" => "more",
        "misc.net" => "Net",
        "misc.total_income" => "Total income",
        "misc.total_expense" => "Total expense",
        "misc.none" => "none",
        "misc.no_accounts" => "no accounts",
        "misc.no_cc_accounts" => "no credit card accounts",
        "misc.no_accounts_cc" => "no accounts with credit card",
        "misc.no_expense_cats" => "no expense categories",
        "misc.locked" => "locked",
        "misc.total" => "TOTAL",
        "misc.free" => "free",
        "misc.created" => "Created",
        "misc.parcela" => "Parcela",
        "misc.is_this_correct" => "Is this correct?",
        "misc.pending" => "(PENDING)",
        "misc.no_sel.transaction" => "No transaction selected.",
        "misc.no_sel.account" => "No account selected.",
        "misc.no_sel.category" => "No category selected.",
        "misc.no_sel.budget" => "No budget selected.",
        "misc.no_sel.installment" => "No installment purchase selected.",
        "misc.no_sel.recurring" => "No recurring transaction selected.",
        "misc.no_sel.transfer" => "No transfer selected.",
        "misc.no_sel.payment" => "No payment selected.",
        "misc.no_sel.statement" => "No statements available.",
        "misc.no_pending_recurring" => "No pending recurring transactions.",
        "misc.no_cc_accts" => "No credit card accounts",
        "misc.no_stmt_selected" => "No statement selected.",
        "misc.due" => "due",

        // ── Export kind labels ──
        "export.accounts" => "accounts",
        "export.budgets" => "budgets",
        "export.categories" => "categories",
        "export.transactions" => "transactions",
        "export.transfers" => "transfers",
        "export.installments" => "installments",
        "export.recurring" => "recurring transactions",
        "export.payments" => "payments",

        // ── Desktop notifications ──
        "notif.no_txn_today" => "You haven't logged any transactions today!",
        "notif.overdue" => "Overdue",
        "notif.budget_exceeded" => "EXCEEDED",
        "notif.budget_reached" => "reached",
        "notif.summary" => "Finances TUI",

        // ── Detail pane labels ──
        "detail.name" => "Name",
        "detail.type" => "Type",
        "detail.billing_day" => "Billing day",
        "detail.due_day" => "Due day",
        "detail.no_credit_card" => "No credit card",
        "detail.payment_methods" => "Payment methods",
        "detail.created" => "Created",
        "detail.spent" => "Spent",
        "detail.total" => "Total",
        "detail.purchase" => "Purchase",
        "detail.amount" => "Amount",
        "detail.next" => "Next",
        "detail.installment" => "Installment",
        "detail.charges" => "Charges",
        "detail.credits" => "Credits",
        "detail.paid" => "Paid",
        "detail.balance" => "Balance",
        "detail.due" => "Due",
        "detail.status" => "Status",

        _ => key,
    }
}

fn t_pt(key: &'static str) -> Option<&'static str> {
    Some(match key {
        // ── Screen labels ──
        "screen.dashboard" => "Painel",
        "screen.transactions" => "Transações",
        "screen.accounts" => "Contas",
        "screen.budgets" => "Orçamentos",
        "screen.categories" => "Categorias",
        "screen.installments" => "Parcelamentos",
        "screen.recurring" => "Recorrentes",
        "screen.transfers" => "Transferências",
        "screen.cc_payments" => "Pag. Cartão",
        "screen.cc_statements" => "Faturas",
        "screen.reports" => "Relatórios",

        // ── Form labels ──
        "form.name" => "Nome",
        "form.name_pt" => "Nome em Português",
        "form.amount" => "Valor",
        "form.date" => "Data",
        "form.description" => "Descrição",
        "form.billing_day" => "Dia Fechamento",
        "form.due_day" => "Dia Vencimento",
        "form.credit_limit" => "Limite de Crédito",
        "form.from" => "De",
        "form.to" => "Para",
        "form.desc" => "Desc",
        "form.purchase_date" => "Data da Compra",
        "form.installments" => "Parcelas",
        "form.next_due" => "Próximo Venc.",
        "form.total_amount" => "Valor Total",
        "form.is_installment" => "Parcelado",

        // ── Form selector/toggle labels ──
        "form.type" => "Tipo",
        "form.account" => "Conta",
        "form.payment" => "Pagamento",
        "form.category" => "Categoria",
        "form.period" => "Período",
        "form.frequency" => "Frequência",
        "form.credit_card" => "Cartão de Crédito",
        "form.debit_card" => "Cartão de Débito",

        // ── Filter labels ──
        "filter.acct" => "Conta",
        "filter.cat" => "Cat",
        "filter.type" => "Tipo",
        "filter.pay" => "Pag",
        "filter.all" => "Todos",

        // ── Table headers ──
        "header.account" => "Conta",
        "header.type" => "Tipo",
        "header.credit_card" => "Cartão Crédito",
        "header.debit" => "Débito",
        "header.checking" => "Saldo",
        "header.credit_used" => "Crédito Usado",
        "header.date" => "Data",
        "header.description" => "Descrição",
        "header.amount" => "Valor",
        "header.category" => "Categoria",
        "header.period" => "Período",
        "header.spent" => "Gasto",
        "header.pct" => "%",
        "header.total" => "Total",
        "header.num_inst" => "# Parc.",
        "header.purchase_date" => "Data Compra",
        "header.frequency" => "Frequência",
        "header.next_due" => "Próximo Venc.",
        "header.from" => "De",
        "header.to" => "Para",
        "header.balance" => "Saldo",
        "header.status" => "Status",
        "header.filters" => "Filtros",

        // ── Block/section titles ──
        "title.accounts" => "Contas",
        "title.account_details" => "Detalhes da Conta",
        "title.new_account" => "Nova Conta",
        "title.edit_account" => "Editar Conta",
        "title.transactions" => "Transações",
        "title.transaction_details" => "Detalhes da Transação",
        "title.new_transaction" => "Nova Transação",
        "title.edit_transaction" => "Editar Transação",
        "title.categories" => "Categorias",
        "title.category_details" => "Detalhes da Categoria",
        "title.new_category" => "Nova Categoria",
        "title.edit_category" => "Editar Categoria",
        "title.budgets" => "Orçamentos",
        "title.budget_details" => "Detalhes do Orçamento",
        "title.new_budget" => "Novo Orçamento",
        "title.edit_budget" => "Editar Orçamento",
        "title.installments" => "Parcelamentos",
        "title.installment_details" => "Detalhes do Parcelamento",
        "title.new_installment" => "Novo Parcelamento",
        "title.edit_installment" => "Editar Parcelamento",
        "title.confirm_installment" => "Confirmar Parcelamento",
        "title.confirm_installment_edit" => "Confirmar Edição do Parcelamento",
        "title.recurring" => "Transações Recorrentes",
        "title.recurring_details" => "Detalhes da Recorrência",
        "title.new_recurring" => "Nova Transação Recorrente",
        "title.edit_recurring" => "Editar Transação Recorrente",
        "title.transfers" => "Transferências",
        "title.transfer_details" => "Detalhes da Transferência",
        "title.new_transfer" => "Nova Transferência",
        "title.cc_payments" => "Pagamentos de Cartão",
        "title.payment_details" => "Detalhes do Pagamento",
        "title.new_cc_payment" => "Novo Pagamento de Cartão",
        "title.cc_statements" => "Faturas",
        "title.statement_details" => "Detalhes da Fatura",
        "title.statement" => "Fatura",
        "title.details" => "Detalhes",
        "title.accounts_balances" => "Saldos das Contas",
        "title.budget_status" => "Status do Orçamento",
        "title.pending_recurring" => "Recorrentes Pendentes",
        "title.current_cc_statements" => "Faturas Abertas",
        "title.confirm" => "Confirmar",
        "title.filter" => "Filtro (Enter=aplicar, Esc=fechar, r=limpar)",
        "title.reports" => "Relatórios",
        "title.expenses_by_category" => "Despesas por Categoria",
        "title.income_by_category" => "Receitas por Categoria",
        "title.monthly_trend" => "Tendência Mensal",
        "title.report_filters" => "Filtros de Relatório (Enter=aplicar, Esc=fechar)",

        // ── Reports ──
        "report.no_data" => "Sem dados para este período.",
        "report.filter.all_accounts" => "Todas as contas",
        "report.filter.all_methods" => "Todos os métodos de pagamento",
        "report.preset.this_month" => "Mês atual",
        "report.preset.last_month" => "Mês anterior",
        "report.preset.last_3_months" => "Últimos 3 meses",
        "report.preset.ytd" => "Ano até hoje",
        "report.preset.this_year" => "Ano atual",
        "report.preset.last_year" => "Ano anterior",
        "report.preset.custom" => "Personalizado",

        // ── Status bar keybinding text ──
        "status.quit" => "Sair",
        "status.screen" => "Tela",
        "status.navigate" => "Navegar",
        "status.cancel" => "Cancelar",
        "status.nav_fields" => "Navegar",
        "status.toggle" => "Alternar",
        "status.submit" => "Enviar",
        "status.close" => "Fechar",
        "status.apply" => "Aplicar",
        "status.cycle" => "Ciclar",
        "status.unread" => "não lidas",
        "status.export_report" => "Relatório exportado",

        // ── Detail pane keybinding hints ──
        "hint.acct" => "[n] Nova  [e] Editar  [d] Desativar  [x] Exportar",
        "hint.cat" => "[n] Nova  [e] Editar  [d] Excluir  [x] Exportar",
        "hint.txn" => "[n] Nova  [e] Editar  [d] Excluir  [f] Filtrar  [r] Limpar  [x] Exportar",
        "hint.budget" => "[n] Novo  [e] Editar  [d] Excluir  [x] Exportar",
        "hint.inst" => "[n] Novo  [e] Editar  [d] Excluir  [x] Exportar",
        "hint.recurring" => "[c] Confirmar pendente  [n] Novo  [e] Editar  [d] Desativar  [x] Exportar",
        "hint.xfer" => "[n] Nova  [d] Excluir  [x] Exportar",
        "hint.cc_pay" => "[n] Novo  [d] Excluir  [x] Exportar",
        "hint.cc_stmt_list" => "[Enter] Ver transações  [p] Pagar  [u] Estornar  [h/l] Trocar conta",
        "hint.cc_stmt_list_all" => "[h/l] Trocar conta",
        "hint.cc_stmt_detail" => "[Esc] Voltar  [Enter] Ir para transação / parcelamento  [j/k] Navegar",
        "hint.reports" => "[Tab] Alternar visão  [p] Alternar preset  [f] Filtros  [j/k] Rolar  [x] Exportar HTML",

        // ── Status messages ──
        "msg.create_account_first" => "Crie uma conta primeiro",
        "msg.create_category_first" => "Crie uma categoria primeiro",
        "msg.create_expense_cat_first" => "Crie uma categoria de despesa primeiro",
        "msg.no_cc_account" => "Nenhuma conta com cartão de crédito disponível",
        "msg.need_two_accounts" => "Necessário pelo menos 2 contas para transferência",
        "msg.installment_edit_hint" => "Pressione [e] para editar ou [d] para excluir o grupo de parcelas",
        "msg.select_account_view" => "Selecione uma conta específica para ver transações",
        "msg.select_account_pay" => "Selecione uma conta específica para pagar fatura",
        "msg.select_account_unpay" => "Selecione uma conta específica para estornar pagamento",
        "msg.cannot_pay_upcoming" => "Não é possível pagar fatura futura",
        "msg.cannot_pay_open" => "Não é possível pagar fatura aberta",
        "msg.already_paid" => "Fatura já está totalmente paga",
        "msg.only_closed_unpay" => "Somente faturas fechadas podem ser estornadas",
        "msg.no_payments" => "Fatura não possui pagamentos",
        "msg.statement_paid" => "Fatura paga",

        // ── Validation errors ──
        "err.name_required" => "Nome é obrigatório",
        "err.description_required" => "Descrição é obrigatória",
        "err.invalid_date" => "Data inválida (use DD-MM-AAAA)",
        "err.invalid_amount" => "Valor inválido",
        "err.amount_positive" => "Valor deve ser positivo",
        "err.credit_limit_positive" => "Limite de crédito deve ser positivo",
        "err.invalid_credit_limit" => "Limite de crédito inválido",
        "err.billing_day_range" => "Dia de fechamento deve ser 1-31",
        "err.due_day_range" => "Dia de vencimento deve ser 1-31",
        "err.no_account" => "Nenhuma conta selecionada",
        "err.no_source_account" => "Nenhuma conta de origem selecionada",
        "err.no_dest_account" => "Nenhuma conta de destino selecionada",
        "err.same_account" => "Origem e destino devem ser contas diferentes",
        "err.no_payment_method" => "Nenhum método de pagamento selecionado",
        "err.no_category" => "Nenhuma categoria selecionada",
        "err.no_period" => "Nenhum período selecionado",
        "err.no_frequency" => "Nenhuma frequência selecionada",
        "err.at_least_two_inst" => "Mínimo de 2 parcelas",
        "err.invalid_inst_count" => "Número de parcelas inválido",
        "err.budget_exists" => "Já existe orçamento para esta categoria e período",
        "err.cannot_change_acct_type" => "Não é possível alterar o tipo: conta possui transações",
        "err.cannot_disable_credit" => "Não é possível desativar cartão de crédito: conta possui transações de crédito",
        "err.cannot_disable_debit" => "Não é possível desativar cartão de débito: conta possui transações de débito",
        "err.cannot_change_cat_type" => "Não é possível alterar o tipo: categoria referenciada por transações ou orçamentos",

        // ── CC Statement status ──
        "stmt.upcoming" => "Futura",
        "stmt.open" => "Aberta",
        "stmt.paid" => "Paga",
        "stmt.due" => "Pendente",
        "stmt.all_accounts" => "Todas as Contas",

        // ── Misc ──
        "misc.yes" => "Sim",
        "misc.no" => "Não",
        "misc.all" => "Todos",
        "misc.more" => "mais",
        "misc.net" => "Saldo",
        "misc.total_income" => "Receita total",
        "misc.total_expense" => "Despesa total",
        "misc.none" => "nenhum",
        "misc.no_accounts" => "sem contas",
        "misc.no_cc_accounts" => "sem contas com cartão",
        "misc.no_accounts_cc" => "sem contas com cartão de crédito",
        "misc.no_expense_cats" => "sem categorias de despesa",
        "misc.locked" => "bloqueado",
        "misc.total" => "TOTAL",
        "misc.free" => "livre",
        "misc.created" => "Criado",
        "misc.parcela" => "Parcela",
        "misc.is_this_correct" => "Está correto?",
        "misc.pending" => "(PENDENTE)",
        "misc.no_sel.transaction" => "Nenhuma transação selecionada.",
        "misc.no_sel.account" => "Nenhuma conta selecionada.",
        "misc.no_sel.category" => "Nenhuma categoria selecionada.",
        "misc.no_sel.budget" => "Nenhum orçamento selecionado.",
        "misc.no_sel.installment" => "Nenhum parcelamento selecionado.",
        "misc.no_sel.recurring" => "Nenhuma transação recorrente selecionada.",
        "misc.no_sel.transfer" => "Nenhuma transferência selecionada.",
        "misc.no_sel.payment" => "Nenhum pagamento selecionado.",
        "misc.no_sel.statement" => "Nenhuma fatura disponível.",
        "misc.no_pending_recurring" => "Nenhuma transação recorrente pendente.",
        "misc.no_cc_accts" => "Nenhuma conta com cartão de crédito",
        "misc.no_stmt_selected" => "Nenhuma fatura selecionada.",
        "misc.due" => "venc.",

        // ── Export kind labels ──
        "export.accounts" => "contas",
        "export.budgets" => "orçamentos",
        "export.categories" => "categorias",
        "export.transactions" => "transações",
        "export.transfers" => "transferências",
        "export.installments" => "parcelamentos",
        "export.recurring" => "transações recorrentes",
        "export.payments" => "pagamentos",

        // ── Desktop notifications ──
        "notif.no_txn_today" => "Você não registrou nenhuma transação hoje!",
        "notif.overdue" => "Atrasado",
        "notif.budget_exceeded" => "EXCEDIDO",
        "notif.budget_reached" => "atingiu",
        "notif.summary" => "Finanças",

        // ── Detail pane labels ──
        "detail.name" => "Nome",
        "detail.type" => "Tipo",
        "detail.billing_day" => "Dia fechamento",
        "detail.due_day" => "Dia vencimento",
        "detail.no_credit_card" => "Sem cartão de crédito",
        "detail.payment_methods" => "Métodos de pagamento",
        "detail.created" => "Criado",
        "detail.spent" => "Gasto",
        "detail.total" => "Total",
        "detail.purchase" => "Compra",
        "detail.amount" => "Valor",
        "detail.next" => "Próximo",
        "detail.installment" => "Parcela",
        "detail.charges" => "Cobranças",
        "detail.credits" => "Créditos",
        "detail.paid" => "Pago",
        "detail.balance" => "Saldo",
        "detail.due" => "Vencimento",
        "detail.status" => "Status",

        _ => return None,
    })
}

// ── Format helpers for dynamic messages ──

/// "Exported {count} {kind} to {path}"
pub fn tf_exported(locale: Locale, count: usize, kind: &str, path: &std::path::Path) -> String {
    match locale {
        Locale::En => format!("Exported {} {} to {}", count, kind, path.display()),
        Locale::Pt => format!("Exportados {} {} para {}", count, kind, path.display()),
    }
}

/// "Export failed: {err}"
pub fn tf_export_failed(locale: Locale, err: &dyn std::fmt::Display) -> String {
    match locale {
        Locale::En => format!("Export failed: {err}"),
        Locale::Pt => format!("Falha na exportação: {err}"),
    }
}

/// "Cannot delete \"{name}\": category is in use"
pub fn tf_cannot_delete_cat(locale: Locale, name: &str) -> String {
    match locale {
        Locale::En => format!("Cannot delete \"{name}\": category is in use"),
        Locale::Pt => format!("Não é possível excluir \"{name}\": categoria em uso"),
    }
}

/// "Deactivate \"{name}\"?" / with references note
pub fn tf_deactivate(locale: Locale, name: &str, has_refs: bool) -> String {
    match (locale, has_refs) {
        (Locale::En, true) => format!("Deactivate \"{name}\"? It has existing transactions/transfers."),
        (Locale::En, false) => format!("Deactivate \"{name}\"?"),
        (Locale::Pt, true) => format!("Desativar \"{name}\"? Possui transações/transferências existentes."),
        (Locale::Pt, false) => format!("Desativar \"{name}\"?"),
    }
}

/// "Delete \"{name}\"?"
pub fn tf_delete(locale: Locale, name: &str) -> String {
    match locale {
        Locale::En => format!("Delete \"{name}\"?"),
        Locale::Pt => format!("Excluir \"{name}\"?"),
    }
}

/// "Delete budget for \"{name}\"?"
pub fn tf_delete_budget(locale: Locale, cat_name: &str) -> String {
    match locale {
        Locale::En => format!("Delete budget for \"{cat_name}\"?"),
        Locale::Pt => format!("Excluir orçamento de \"{cat_name}\"?"),
    }
}

/// "Delete \"{name}\" and all its transactions?"
pub fn tf_delete_installment(locale: Locale, desc: &str) -> String {
    match locale {
        Locale::En => format!("Delete \"{desc}\" and all its transactions?"),
        Locale::Pt => format!("Excluir \"{desc}\" e todas as suas transações?"),
    }
}

/// "Delete transfer \"{desc}\"?"
pub fn tf_delete_transfer(locale: Locale, desc: &str) -> String {
    match locale {
        Locale::En => format!("Delete transfer \"{desc}\"?"),
        Locale::Pt => format!("Excluir transferência \"{desc}\"?"),
    }
}

/// "Delete payment \"{desc}\"?"
pub fn tf_delete_payment(locale: Locale, desc: &str) -> String {
    match locale {
        Locale::En => format!("Delete payment \"{desc}\"?"),
        Locale::Pt => format!("Excluir pagamento \"{desc}\"?"),
    }
}

/// "Not due yet (next due: {date})"
pub fn tf_not_due_yet(locale: Locale, date: &str) -> String {
    match locale {
        Locale::En => format!("Not due yet (next due: {date})"),
        Locale::Pt => format!("Ainda não venceu (próximo: {date})"),
    }
}

/// "Confirmed \"{desc}\" — next due: {date}"
pub fn tf_confirmed(locale: Locale, desc: &str, date: &str) -> String {
    match locale {
        Locale::En => format!("Confirmed \"{desc}\" — next due: {date}"),
        Locale::Pt => format!("Confirmado \"{desc}\" — próximo: {date}"),
    }
}

/// "Pay statement {label}? ({amount})"
pub fn tf_pay_statement(locale: Locale, label: &str, amount: &str) -> String {
    match locale {
        Locale::En => format!("Pay statement {label}? ({amount})"),
        Locale::Pt => format!("Pagar fatura {label}? ({amount})"),
    }
}

/// "Remove all payments for {label}? ({amount})"
pub fn tf_unpay_statement(locale: Locale, label: &str, amount: &str) -> String {
    match locale {
        Locale::En => format!("Remove all payments for {label}? ({amount})"),
        Locale::Pt => format!("Remover todos os pagamentos de {label}? ({amount})"),
    }
}

/// "Removed {n} payment(s)"
pub fn tf_removed_payments(locale: Locale, n: u64) -> String {
    match locale {
        Locale::En => format!("Removed {} payment{}", n, if n == 1 { "" } else { "s" }),
        Locale::Pt => format!("{} pagamento{} removido{}", n, if n == 1 { "" } else { "s" }, if n == 1 { "" } else { "s" }),
    }
}

/// "Notifications ({n} unread) — r: dismiss  R: dismiss all"
pub fn tf_notifications_title(locale: Locale, n: usize) -> String {
    match locale {
        Locale::En => format!("Notifications ({n} unread) — r: dismiss  R: dismiss all"),
        Locale::Pt => format!("Notificações ({n} não lidas) — r: dispensar  R: dispensar todas"),
    }
}

/// Paginated title: "Transactions (1-50 of 200)"
pub fn tf_paginated(locale: Locale, label: &str, start: u64, end: u64, total: u64) -> String {
    let of = match locale {
        Locale::En => "of",
        Locale::Pt => "de",
    };
    format!("{label} ({start}-{end} {of} {total})")
}

/// "Transactions ({count})" for detail view
pub fn tf_count_title(label: &str, count: usize) -> String {
    format!("{label} ({count})")
}
