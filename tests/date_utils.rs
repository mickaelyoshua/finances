use chrono::NaiveDate;
use finances::db::{
    clamped_day, last_day_of_month, latest_closing_date, next_month, prev_month,
    recurring::compute_next_due, statement_due_date, statement_period,
};
use finances::models::Frequency;
use finances::ui::screens::cc_statements::CreditCardStatement;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// -- add_months (tested indirectly via compute_next_due with Monthly) --

#[test]
fn next_due_daily() {
    let date = NaiveDate::from_ymd_opt(2026, 3, 13).unwrap();
    assert_eq!(
        compute_next_due(date, Frequency::Daily),
        NaiveDate::from_ymd_opt(2026, 3, 14).unwrap()
    );
}

#[test]
fn next_due_weekly() {
    let date = NaiveDate::from_ymd_opt(2026, 3, 13).unwrap();
    assert_eq!(
        compute_next_due(date, Frequency::Weekly),
        NaiveDate::from_ymd_opt(2026, 3, 20).unwrap()
    );
}

#[test]
fn next_due_monthly_normal() {
    let date = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
    assert_eq!(
        compute_next_due(date, Frequency::Monthly),
        NaiveDate::from_ymd_opt(2026, 4, 15).unwrap()
    );
}

#[test]
fn next_due_monthly_jan31_to_feb28() {
    let date = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
    assert_eq!(
        compute_next_due(date, Frequency::Monthly),
        NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
    );
}

#[test]
fn next_due_monthly_dec_to_jan_year_rollover() {
    let date = NaiveDate::from_ymd_opt(2026, 12, 15).unwrap();
    assert_eq!(
        compute_next_due(date, Frequency::Monthly),
        NaiveDate::from_ymd_opt(2027, 1, 15).unwrap()
    );
}

#[test]
fn next_due_yearly() {
    let date = NaiveDate::from_ymd_opt(2026, 3, 13).unwrap();
    assert_eq!(
        compute_next_due(date, Frequency::Yearly),
        NaiveDate::from_ymd_opt(2027, 3, 13).unwrap()
    );
}

#[test]
fn next_due_yearly_leap_day() {
    // Feb 29 in a leap year → Feb 28 next year
    let date = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
    assert_eq!(
        compute_next_due(date, Frequency::Yearly),
        NaiveDate::from_ymd_opt(2025, 2, 28).unwrap()
    );
}

#[test]
fn next_due_monthly_feb28_to_mar28() {
    // Feb 28 (non-leap) → Mar 28 (not Mar 31)
    let date = NaiveDate::from_ymd_opt(2026, 2, 28).unwrap();
    assert_eq!(
        compute_next_due(date, Frequency::Monthly),
        NaiveDate::from_ymd_opt(2026, 3, 28).unwrap()
    );
}

// -- prev_month / next_month --

#[test]
fn prev_month_normal() {
    assert_eq!(prev_month(2026, 6), (2026, 5));
}

#[test]
fn prev_month_january_wraps_to_december() {
    assert_eq!(prev_month(2026, 1), (2025, 12));
}

#[test]
fn next_month_normal() {
    assert_eq!(next_month(2026, 6), (2026, 7));
}

#[test]
fn next_month_december_wraps_to_january() {
    assert_eq!(next_month(2026, 12), (2027, 1));
}

// -- last_day_of_month --

#[test]
fn last_day_of_month_feb_non_leap() {
    assert_eq!(last_day_of_month(2026, 2), 28);
}

#[test]
fn last_day_of_month_feb_leap() {
    assert_eq!(last_day_of_month(2024, 2), 29);
}

#[test]
fn last_day_of_month_31_day_months() {
    for m in [1, 3, 5, 7, 8, 10, 12] {
        assert_eq!(last_day_of_month(2026, m), 31, "month {m}");
    }
}

#[test]
fn last_day_of_month_30_day_months() {
    for m in [4, 6, 9, 11] {
        assert_eq!(last_day_of_month(2026, m), 30, "month {m}");
    }
}

// -- clamped_day --

#[test]
fn clamped_day_within_range() {
    assert_eq!(
        clamped_day(2026, 3, 15),
        NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()
    );
}

#[test]
fn clamped_day_31_in_february() {
    assert_eq!(
        clamped_day(2026, 2, 31),
        NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
    );
}

#[test]
fn clamped_day_31_in_april() {
    assert_eq!(
        clamped_day(2026, 4, 31),
        NaiveDate::from_ymd_opt(2026, 4, 30).unwrap()
    );
}

#[test]
fn clamped_day_29_in_leap_feb() {
    assert_eq!(
        clamped_day(2024, 2, 29),
        NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
    );
}

// -- latest_closing_date --

#[test]
fn latest_closing_date_today_is_billing_day() {
    let today = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
    assert_eq!(
        latest_closing_date(today, 15),
        NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()
    );
}

#[test]
fn latest_closing_date_today_after_billing_day() {
    let today = NaiveDate::from_ymd_opt(2026, 3, 20).unwrap();
    assert_eq!(
        latest_closing_date(today, 15),
        NaiveDate::from_ymd_opt(2026, 3, 15).unwrap()
    );
}

#[test]
fn latest_closing_date_today_before_billing_day() {
    let today = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
    assert_eq!(
        latest_closing_date(today, 15),
        NaiveDate::from_ymd_opt(2026, 2, 15).unwrap()
    );
}

#[test]
fn latest_closing_date_billing_31_in_feb() {
    // billing_day=31, today is March 5 → latest close was Feb 28
    let today = NaiveDate::from_ymd_opt(2026, 3, 5).unwrap();
    assert_eq!(
        latest_closing_date(today, 31),
        NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
    );
}

#[test]
fn latest_closing_date_year_boundary() {
    // billing_day=15, today is Jan 5 → latest close was Dec 15 previous year
    let today = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
    assert_eq!(
        latest_closing_date(today, 15),
        NaiveDate::from_ymd_opt(2025, 12, 15).unwrap()
    );
}

// -- statement_period --

#[test]
fn statement_period_normal() {
    let close = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
    let (start, end) = statement_period(close, 15);
    assert_eq!(start, NaiveDate::from_ymd_opt(2026, 2, 16).unwrap());
    assert_eq!(end, NaiveDate::from_ymd_opt(2026, 3, 15).unwrap());
}

#[test]
fn statement_period_billing_31_feb() {
    // close = Feb 28 (clamped from 31), prev close = Jan 31
    let close = NaiveDate::from_ymd_opt(2026, 2, 28).unwrap();
    let (start, end) = statement_period(close, 31);
    assert_eq!(start, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
    assert_eq!(end, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
}

#[test]
fn statement_period_billing_31_march() {
    // close = Mar 31, prev close = Feb 28 (clamped from 31)
    let close = NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
    let (start, end) = statement_period(close, 31);
    assert_eq!(start, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap());
    assert_eq!(end, NaiveDate::from_ymd_opt(2026, 3, 31).unwrap());
}

#[test]
fn statement_period_year_boundary() {
    let close = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let (start, end) = statement_period(close, 15);
    assert_eq!(start, NaiveDate::from_ymd_opt(2025, 12, 16).unwrap());
    assert_eq!(end, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
}

// -- statement_due_date --

#[test]
fn due_date_due_after_billing() {
    // billing_day=15, due_day=25 → due in same month as close
    assert_eq!(
        statement_due_date(2026, 3, 15, 25),
        NaiveDate::from_ymd_opt(2026, 3, 25).unwrap()
    );
}

#[test]
fn due_date_due_before_billing() {
    // billing_day=25, due_day=10 → due in next month
    assert_eq!(
        statement_due_date(2026, 3, 25, 10),
        NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()
    );
}

#[test]
fn due_date_due_equals_billing() {
    // billing_day=15, due_day=15 → due_day <= billing_day → next month
    assert_eq!(
        statement_due_date(2026, 3, 15, 15),
        NaiveDate::from_ymd_opt(2026, 4, 15).unwrap()
    );
}

#[test]
fn due_date_december_wraps_to_january() {
    // billing_day=25, due_day=10, close in December → due in January next year
    assert_eq!(
        statement_due_date(2026, 12, 25, 10),
        NaiveDate::from_ymd_opt(2027, 1, 10).unwrap()
    );
}

#[test]
fn due_date_same_month_clamped() {
    // billing_day=5, due_day=30 → due in same month; in February, day 30 clamps to 28
    assert_eq!(
        statement_due_date(2026, 2, 5, 30),
        NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
    );
}

// -- latest_closing_date edge case --

#[test]
fn latest_closing_date_billing_day_1() {
    // billing_day=1: today is the 1st → close is today
    let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    assert_eq!(
        latest_closing_date(today, 1),
        NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()
    );
}

#[test]
fn latest_closing_date_billing_day_1_mid_month() {
    // billing_day=1, today is the 15th → close is the 1st of current month
    let today = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
    assert_eq!(
        latest_closing_date(today, 1),
        NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()
    );
}

// -- statement_period consecutive: no gaps, no overlaps --

#[test]
fn statement_periods_are_contiguous() {
    // 3 consecutive periods for billing_day=15 should chain without gaps
    let close1 = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
    let close2 = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    let close3 = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();

    let (_, e1) = statement_period(close1, 15);
    let (s2, e2) = statement_period(close2, 15);
    let (s3, _) = statement_period(close3, 15);

    // period 2 starts the day after period 1 ends
    assert_eq!(s2, e1.succ_opt().unwrap());
    // period 3 starts the day after period 2 ends
    assert_eq!(s3, e2.succ_opt().unwrap());
}

#[test]
fn statement_periods_contiguous_billing_31() {
    // billing_day=31 across Jan(31), Feb(28), Mar(31) — clamping must not create gaps
    let close_jan = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
    let close_feb = NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(); // clamped from 31
    let close_mar = NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();

    let (_, e1) = statement_period(close_jan, 31);
    let (s2, e2) = statement_period(close_feb, 31);
    let (s3, _) = statement_period(close_mar, 31);

    assert_eq!(s2, e1.succ_opt().unwrap());
    assert_eq!(s3, e2.succ_opt().unwrap());
}

// -- CreditCardStatement --

fn make_statement(total: Decimal, paid: Decimal, is_current: bool) -> CreditCardStatement {
    CreditCardStatement {
        period_start: NaiveDate::from_ymd_opt(2026, 2, 16).unwrap(),
        period_end: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
        due_date: NaiveDate::from_ymd_opt(2026, 3, 25).unwrap(),
        total_charges: total.max(Decimal::ZERO),
        total_credits: Decimal::ZERO,
        statement_total: total,
        paid_amount: paid,
        is_current,
        is_upcoming: false,
    }
}

#[test]
fn balance_due_normal() {
    let stmt = make_statement(dec!(500), dec!(200), false);
    assert_eq!(stmt.balance_due(), dec!(300));
}

#[test]
fn balance_due_fully_paid() {
    let stmt = make_statement(dec!(500), dec!(500), false);
    assert_eq!(stmt.balance_due(), Decimal::ZERO);
}

#[test]
fn balance_due_overpaid_clamps_to_zero() {
    let stmt = make_statement(dec!(500), dec!(600), false);
    assert_eq!(stmt.balance_due(), Decimal::ZERO);
}

#[test]
fn balance_due_zero_statement() {
    let stmt = make_statement(Decimal::ZERO, Decimal::ZERO, false);
    assert_eq!(stmt.balance_due(), Decimal::ZERO);
}

#[test]
fn status_label_open() {
    let stmt = make_statement(dec!(100), Decimal::ZERO, true);
    assert_eq!(stmt.status_label(), "Open");
}

#[test]
fn status_label_open_even_if_zero() {
    // Current statement with zero balance is still "Open", not "Paid"
    let stmt = make_statement(Decimal::ZERO, Decimal::ZERO, true);
    assert_eq!(stmt.status_label(), "Open");
}

#[test]
fn status_label_paid() {
    let stmt = make_statement(dec!(500), dec!(500), false);
    assert_eq!(stmt.status_label(), "Paid");
}

#[test]
fn status_label_due() {
    let stmt = make_statement(dec!(500), dec!(200), false);
    assert_eq!(stmt.status_label(), "Due");
}

#[test]
fn status_label_upcoming() {
    let mut stmt = make_statement(dec!(500), Decimal::ZERO, false);
    stmt.is_upcoming = true;
    assert_eq!(stmt.status_label(), "Upcoming");
}

#[test]
fn label_format() {
    let stmt = make_statement(dec!(100), Decimal::ZERO, false);
    // period_end is 2026-03-15 → "03/2026"
    assert_eq!(stmt.label(), "03/2026");
}
