# Finances — Developer Guide

Personal finance TUI in Rust. Production database on Neon (PostgreSQL), local dev via docker-compose.

## Build & Run

```sh
docker compose up -d          # start local PostgreSQL
cargo build                   # compile
cargo run                     # TUI (local dev DB)
cargo run -- --prod           # TUI (production Neon DB)
cargo run -- --migrate        # run migrations (dev)
cargo run -- --migrate --prod # run migrations (prod)
cargo run -- --notify --prod  # send desktop notifications (for cron)
cargo test                    # run tests (needs local DB running)
```

## Environment

- `.env` — local dev database URL (docker-compose)
- `.env.prod` — production database URL (Neon, TLS required)
- `.env.example` / `.env.prod.example` — templates tracked in git
- `--prod` flag switches between `.env` and `.env.prod`

## Architecture

- **Pattern**: Elm/TEA — Event → Update (`handle_key`) → Render (`draw`)
- **App struct**: single source of truth, caches DB data, refreshes after mutations
- **Screens**: 9 screens (Dashboard, Transactions, Accounts, Categories, Budgets, Installments, Recurring, Transfers, CC Payments)
- **InputMode**: `Normal` (navigation) vs `Editing` (form input)
- **ConfirmAction enum**: tracks what a popup confirmation will do
- **Forms**: per-screen form structs with validation; early-return for Esc/Enter before `if let Some(form)` to avoid borrow conflicts

## Key Conventions

- **Money**: always `rust_decimal::Decimal`, never `f64`
- **Currency**: BRL (R$), formatted via `format_brl()` with dot thousands / comma decimal
- **Enums**: all implement `Display` + `FromStr`; DB functions accept enum types, not `&str`
- **Balances**: computed from transactions + transfers + credit_card_payments, never stored
- **Migrations**: `sqlx::migrate!()` — idempotent, tracked in `_sqlx_migrations` table
- **Logging**: tracing with daily-rotating file appender in `~/.local/share/finances/`
- **Exports**: CSV files written to `~/.local/share/finances/exports/`

## Database

- 9 tables: accounts, categories, transactions, transfers, credit_card_payments, installment_purchases, budgets, recurring_transactions, notifications
- Local: `finances/finances@localhost:5432/finances` (docker-compose)
- Production: Neon free tier with TLS (`tls-rustls` feature in sqlx)
- Single migration file: `migrations/20260305_initial.sql`
