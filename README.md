# Finances

A personal finance tracker with a terminal UI, built in Rust.

Track transactions, transfers, credit card payments, budgets, and recurring expenses — all from the terminal, backed by PostgreSQL.

## Features

- **Accounts** — checking accounts, cash wallets, with optional credit/debit cards
- **Transactions** — income and expenses with category, payment method, and date
- **Credit cards** — billing cycle tracking, installment purchases (parcelas), card bill payments
- **Transfers** — move money between accounts
- **Budgets** — weekly, monthly, or yearly spending limits per category
- **Recurring transactions** — automatic reminders for bills and subscriptions
- **Desktop notifications** — get reminded if you haven't logged anything today
- **TUI** — keyboard-driven interface with tab navigation (powered by ratatui)

## Tech Stack

| Component | Library |
|-----------|---------|
| TUI | ratatui 0.30 + crossterm 0.29 |
| Database | PostgreSQL 17 via sqlx 0.8 (async) |
| Money | rust_decimal (never floating-point) |
| Async | tokio |
| CLI | clap 4 |
| Error handling | anyhow |

## Prerequisites

- Rust (edition 2024)
- Docker & Docker Compose (for local PostgreSQL)

## Getting Started

### 1. Start the database

```sh
docker compose up -d
```

This starts a PostgreSQL 17 container on `localhost:5432` (user: `finances`, password: `finances`, database: `finances`).

### 2. Configure the connection

Create a `.env` file in the project root:

```
DATABASE_URL=postgres://finances:finances@localhost:5432/finances
```

### 3. Run migrations

```sh
cargo run -- --migrate
```

This creates all tables and seeds default categories and a cash account.

### 4. (Optional) Load seed data

```sh
psql postgres://finances:finances@localhost:5432/finances < seeds.sql
```

### 5. Launch the TUI

```sh
cargo run
```

## Usage

| Key | Action |
|-----|--------|
| `1`–`7` / `← →` | Switch screens |
| `↑ ↓` | Navigate lists |
| `n` | New item |
| `e` | Edit selected |
| `d` | Delete/deactivate selected |
| `q` | Quit |

### CLI Flags

```
--migrate   Run database migrations and exit
--notify    Show a desktop notification if no transactions were logged today
```

## Project Structure

```
src/
├── main.rs              # Entry point, terminal setup, main loop
├── config.rs            # CLI args and DB URL
├── models/              # Domain types (Account, Transaction, Budget, ...)
├── db/                  # Database operations (one module per table)
└── ui/                  # TUI layer
    ├── app.rs           # App state, key handling, confirm actions
    ├── event.rs         # Keyboard/tick event handler
    ├── render.rs        # Screen rendering
    ├── components/      # Reusable widgets (input, popup, toggle, format)
    └── screens/         # Per-screen UI logic
migrations/
    └── 20260305_initial.sql
```

## License

[MIT](LICENSE)
