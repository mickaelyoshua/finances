use chrono::NaiveDate;
use finances::db::recurring::compute_next_due;
use finances::models::Frequency;

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
