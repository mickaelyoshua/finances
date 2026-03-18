# Finances

A personal finance tracker with a terminal UI, built in Rust.

Track transactions, transfers, credit card payments, budgets, and recurring expenses — all from the terminal, backed by PostgreSQL.

## Features

- **9 screens** — Dashboard, Transactions, Accounts, Categories, Budgets, Installments, Recurring, Transfers, CC Payments
- **Accounts** — checking accounts and cash wallets, with optional credit/debit cards
- **Transactions** — income and expenses with category, payment method, and date
- **Credit cards** — billing cycle tracking, installment purchases (parcelas), card bill payments
- **Transfers** — move money between accounts
- **Budgets** — weekly, monthly, or yearly spending limits per category with color-coded utilization
- **Recurring transactions** — automatic reminders for bills and subscriptions
- **Installment purchases** — split credit card purchases into N monthly parcelas
- **Transaction filtering** — filter by date range, account, category, type, payment method, or description
- **Pagination** — navigate large transaction lists with PgUp/PgDn
- **CSV export** — export any screen's data to `~/.local/share/finances/exports/`
- **Persistent notifications** — alerts for missing transactions, overdue recurring, and budget thresholds
- **Desktop notifications** — system popup via `--notify` flag (designed for cron)
- **Dev/prod switching** — `--prod` flag connects to production database
- **TUI** — keyboard-driven interface with tab navigation (powered by ratatui)

## Tech Stack

| Component | Library |
|-----------|---------|
| TUI | ratatui 0.30 + crossterm 0.29 |
| Database | PostgreSQL 17 via sqlx 0.8 (async) |
| Money | rust_decimal (never floating-point) |
| Async | tokio |
| CLI | clap 4 |
| CSV | csv 1 |
| Notifications | notify-rust 4 |
| Logging | tracing + tracing-appender (daily rotation) |
| Error handling | anyhow |

## Prerequisites

- Rust (edition 2024)
- Docker & Docker Compose (for local PostgreSQL)

## Getting Started

### 1. Start the local database

```sh
docker compose up -d
```

This starts a PostgreSQL 17 container on `localhost:5432` (user: `finances`, password: `finances`, database: `finances`).

### 2. Configure the connection

Copy `.env.example` to `.env`:

```sh
cp .env.example .env
```

For production (Neon), copy `.env.prod.example` to `.env.prod` and fill in your credentials:

```sh
cp .env.prod.example .env.prod
```

### 3. Run migrations

```sh
cargo run -- --migrate
```

For production:

```sh
cargo run -- --migrate --prod
```

### 4. (Optional) Load seed data

```sh
psql postgres://finances:finances@localhost:5432/finances < seeds.sql
```

### 5. Launch the TUI

```sh
cargo run           # local dev database
cargo run -- --prod # production database (Neon)
```

## Usage

| Key | Action |
|-----|--------|
| `1`–`9` / `← →` | Switch screens |
| `↑ ↓` / `j k` | Navigate lists |
| `n` | New item |
| `e` | Edit selected |
| `d` | Delete/deactivate selected |
| `f` | Toggle transaction filters |
| `x` | Export current screen to CSV |
| `c` | Confirm pending recurring transaction |
| `r` / `R` | Dismiss one / all notifications |
| `PgUp` / `PgDn` | Previous / next page (transactions) |
| `Esc` | Cancel form / close filter |
| `q` | Quit |

### CLI Flags

```
--migrate   Run database migrations and exit
--notify    Check for alerts and send a desktop notification
--prod      Connect to production database (.env.prod)
```

### Automated Notifications

Set up a cron job to get daily reminders:

```sh
crontab -e
# Example: check every day at 20:00
0 20 * * * /path/to/finances --notify --prod
```

## Project Structure

```
src/
├── main.rs              # Entry point, terminal setup, main loop
├── lib.rs               # Crate root (re-exports modules)
├── config.rs            # CLI args (clap) and DB URL resolution
├── export.rs            # CSV export functions for all screens
├── models/              # Domain types
│   ├── mod.rs
│   ├── account.rs       # Account, AccountType
│   ├── transaction.rs   # Transaction, TransactionType, PaymentMethod
│   ├── budget.rs        # Budget, BudgetPeriod
│   ├── category.rs      # Category, CategoryType
│   ├── recurring.rs     # RecurringTransaction, Frequency
│   ├── installment.rs   # InstallmentPurchase
│   ├── transfer.rs      # Transfer
│   ├── credit_card_payment.rs
│   └── notification.rs  # Notification, NotificationType
├── db/                  # Database operations (one module per table)
│   ├── mod.rs           # create_pool, run_migrations, date helpers
│   ├── accounts.rs      # CRUD + balance computation
│   ├── transactions.rs  # CRUD + filtering + pagination
│   ├── budgets.rs       # CRUD + batch spending computation
│   ├── categories.rs    # CRUD + reference checks
│   ├── recurring.rs     # CRUD + pending list + next_due
│   ├── installments.rs  # Transactional create (N transactions)
│   ├── transfers.rs     # CRUD
│   ├── credit_card_payments.rs
│   └── notifications.rs # Upsert, list unread, mark read
└── ui/                  # TUI layer
    ├── mod.rs
    ├── app.rs           # App state, key handling, confirm actions
    ├── event.rs         # Keyboard/tick event handler
    ├── render.rs        # Tab bar, content dispatch, status bar
    ├── components/      # Reusable widgets
    │   ├── mod.rs
    │   ├── input.rs     # InputField (UTF-8-safe cursor)
    │   ├── popup.rs     # ConfirmPopup (Yes/No dialog)
    │   ├── toggle.rs    # Cycling selector widget
    │   └── format.rs    # format_brl (R$ formatting)
    └── screens/         # Per-screen UI logic
        ├── mod.rs
        ├── dashboard.rs
        ├── transactions.rs
        ├── accounts.rs
        ├── categories.rs
        ├── budgets.rs
        ├── installments.rs
        ├── recurring.rs
        ├── transfers.rs
        └── cc_payments.rs
migrations/
    └── 20260305_initial.sql
tests/
    ├── export.rs
    ├── db.rs
    ├── models.rs
    ├── format.rs
    ├── input.rs
    ├── date_utils.rs
    ├── transactions_filter.rs
    └── transactions_form.rs
```

## License

[MIT](LICENSE)
