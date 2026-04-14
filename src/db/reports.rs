//! Report aggregation queries.
//!
//! Transactions paid by credit card are attributed to the *statement payment
//! date* (period_end + 1), not the raw transaction date, so the report
//! reflects when cash actually leaves the account. Installments already
//! materialise as separate transactions with their own per-installment date,
//! so they map the same way as any credit transaction.

use chrono::{Datelike, NaiveDate};
use sqlx::{PgPool, Postgres, QueryBuilder};

use crate::db::{clamped_day, latest_closing_date, statement_period};
use crate::models::{Account, PaymentMethod, Transaction};

/// Maximum number of days the statement-payment date can shift a transaction
/// forward of its raw date. A statement closing on day 1 with a tx on day 2
/// pays near a month later, so 62 days is a safe upper bound.
const CC_LOOKBACK_DAYS: i64 = 62;

/// Compute the "effective" date a transaction should be attributed to for
/// reports:
///
/// - Non-credit payments → the transaction's own date.
/// - Credit payments → the statement payment date for the cycle that
///   contained the transaction (`period_end + 1`).
///
/// Falls back to `tx.date` if the account has no CC config or doesn't exist.
pub fn effective_date(tx: &Transaction, account: Option<&Account>) -> NaiveDate {
    if tx.parsed_payment_method() != PaymentMethod::Credit {
        return tx.date;
    }
    let Some(acc) = account else { return tx.date };
    if !acc.has_credit_card {
        return tx.date;
    }
    let Some(billing_day) = acc.billing_day else {
        return tx.date;
    };
    let billing_day = billing_day as u32;

    // Find the statement cycle containing tx.date, then return the day after
    // the close date (= payment date, per cc_statements.rs:688).
    let close = next_close_on_or_after(tx.date, billing_day);
    let (start, end) = statement_period(close, billing_day);
    debug_assert!(start <= tx.date && tx.date <= end, "tx should fall inside cycle");
    let _ = (start, end);
    close.succ_opt().unwrap_or(tx.date)
}

/// Find the closing date of the statement cycle that contains `date`.
/// Cycle is (prev_close+1 .. close] — inclusive of the close date itself.
fn next_close_on_or_after(date: NaiveDate, billing_day: u32) -> NaiveDate {
    let latest = latest_closing_date(date, billing_day);
    if latest == date {
        return latest;
    }
    // latest < date → the cycle that contains `date` closes in this or next month.
    let close_this = clamped_day(date.year(), date.month(), billing_day);
    if close_this >= date {
        close_this
    } else {
        let (ny, nm) = crate::db::next_month(date.year(), date.month());
        clamped_day(ny, nm, billing_day)
    }
}

/// Fetch transactions that might be attributable to [start, end] after CC mapping.
/// Widens the raw-date range backward so credit txs whose effective date lands
/// in the requested window are included even when their raw date sits earlier.
///
/// `account_id=None` / `payment_method=None` = no constraint on that dimension.
pub async fn fetch_transactions_for_report(
    pool: &PgPool,
    start: NaiveDate,
    end: NaiveDate,
    account_id: Option<i32>,
    payment_method: Option<PaymentMethod>,
) -> Result<Vec<Transaction>, sqlx::Error> {
    let lookback_start = start
        .checked_sub_signed(chrono::Duration::days(CC_LOOKBACK_DAYS))
        .unwrap_or(start);

    let mut qb = QueryBuilder::<Postgres>::new("SELECT * FROM transactions WHERE date >= ");
    qb.push_bind(lookback_start);
    qb.push(" AND date <= ");
    qb.push_bind(end);
    if let Some(id) = account_id {
        qb.push(" AND account_id = ").push_bind(id);
    }
    if let Some(m) = payment_method {
        qb.push(" AND payment_method = ")
            .push_bind(m.as_str().to_string());
    }
    qb.push(" ORDER BY date ASC, id ASC");
    qb.build_query_as::<Transaction>().fetch_all(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn cc_account(billing_day: i16) -> Account {
        Account {
            id: 1,
            name: "Test".into(),
            account_type: "checking".into(),
            has_credit_card: true,
            credit_limit: Some(dec!(1000)),
            billing_day: Some(billing_day),
            due_day: Some(10),
            has_debit_card: false,
            active: true,
            created_at: Utc::now(),
        }
    }

    fn tx(date: NaiveDate, method: &str) -> Transaction {
        Transaction {
            id: 1,
            amount: dec!(100),
            description: "x".into(),
            category_id: 1,
            account_id: 1,
            transaction_type: "expense".into(),
            payment_method: method.into(),
            date,
            installment_purchase_id: None,
            installment_number: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn effective_date_non_credit_returns_tx_date() {
        let acc = cc_account(20);
        let t = tx(NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(), "pix");
        assert_eq!(
            effective_date(&t, Some(&acc)),
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()
        );
    }

    #[test]
    fn effective_date_credit_before_close_pays_next_month() {
        // billing_day=20 → cycle ends Apr 20. tx on Apr 10 should pay Apr 21.
        let acc = cc_account(20);
        let t = tx(NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(), "credit");
        assert_eq!(
            effective_date(&t, Some(&acc)),
            NaiveDate::from_ymd_opt(2026, 4, 21).unwrap()
        );
    }

    #[test]
    fn effective_date_credit_on_close_day_belongs_to_current_cycle() {
        // tx on the close day (Apr 20) — should pay Apr 21 (same cycle closes that day).
        let acc = cc_account(20);
        let t = tx(NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(), "credit");
        assert_eq!(
            effective_date(&t, Some(&acc)),
            NaiveDate::from_ymd_opt(2026, 4, 21).unwrap()
        );
    }

    #[test]
    fn effective_date_credit_after_close_rolls_to_next_cycle() {
        // tx on Apr 21 is after the Apr 20 close → belongs to the May 20 cycle → pays May 21.
        let acc = cc_account(20);
        let t = tx(NaiveDate::from_ymd_opt(2026, 4, 21).unwrap(), "credit");
        assert_eq!(
            effective_date(&t, Some(&acc)),
            NaiveDate::from_ymd_opt(2026, 5, 21).unwrap()
        );
    }

    #[test]
    fn effective_date_credit_no_account_falls_back_to_tx_date() {
        let t = tx(NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(), "credit");
        assert_eq!(
            effective_date(&t, None),
            NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()
        );
    }
}
