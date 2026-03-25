//! Personal finance manager — a TUI for tracking transactions, budgets,
//! recurring bills, installment purchases, and credit card statements.
//!
//! The binary runs in three modes selected by CLI flags:
//! - **TUI** (default) — interactive terminal interface (ratatui + crossterm)
//! - **`--migrate`** — run database migrations and exit
//! - **`--notify`** — check for alerts (no transactions today, overdue recurring,
//!   over-budget) and send a desktop notification via `notify-rust`
//!
//! All money values use [`rust_decimal::Decimal`]; currency is BRL (R$).

pub mod config;
pub mod db;
pub mod export;
pub mod models;
pub mod ui;
