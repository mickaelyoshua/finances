use std::fmt;

use chrono::{Datelike, Local, NaiveDate};
use rust_decimal::Decimal;

use super::PaymentMethod;
use crate::db::{last_day_of_month, next_month, prev_month};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportView {
    ExpensesByCategory,
    IncomeByCategory,
    MonthlyTrend,
}

impl ReportView {
    pub const ALL: [Self; 3] = [
        Self::ExpensesByCategory,
        Self::IncomeByCategory,
        Self::MonthlyTrend,
    ];

    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::ExpensesByCategory => "title.expenses_by_category",
            Self::IncomeByCategory => "title.income_by_category",
            Self::MonthlyTrend => "title.monthly_trend",
        }
    }

    pub fn cycle_next(self) -> Self {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn cycle_prev(self) -> Self {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodPreset {
    ThisMonth,
    LastMonth,
    Last3Months,
    YearToDate,
    ThisYear,
    LastYear,
    Custom,
}

impl PeriodPreset {
    pub const ALL: [Self; 7] = [
        Self::ThisMonth,
        Self::LastMonth,
        Self::Last3Months,
        Self::YearToDate,
        Self::ThisYear,
        Self::LastYear,
        Self::Custom,
    ];

    pub fn i18n_key(self) -> &'static str {
        match self {
            Self::ThisMonth => "report.preset.this_month",
            Self::LastMonth => "report.preset.last_month",
            Self::Last3Months => "report.preset.last_3_months",
            Self::YearToDate => "report.preset.ytd",
            Self::ThisYear => "report.preset.this_year",
            Self::LastYear => "report.preset.last_year",
            Self::Custom => "report.preset.custom",
        }
    }

    pub fn cycle_next(self) -> Self {
        let i = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    /// Resolve this preset to a concrete (start, end) date range using `today`.
    /// Returns `None` for `Custom` (caller supplies the range).
    pub fn resolve(self, today: NaiveDate) -> Option<(NaiveDate, NaiveDate)> {
        let (y, m) = (today.year(), today.month());
        match self {
            Self::ThisMonth => {
                let start = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
                let end = NaiveDate::from_ymd_opt(y, m, last_day_of_month(y, m)).unwrap();
                Some((start, end))
            }
            Self::LastMonth => {
                let (py, pm) = prev_month(y, m);
                let start = NaiveDate::from_ymd_opt(py, pm, 1).unwrap();
                let end = NaiveDate::from_ymd_opt(py, pm, last_day_of_month(py, pm)).unwrap();
                Some((start, end))
            }
            Self::Last3Months => {
                // From first day of the month 2 months back through the last day of the current month.
                let (mut y0, mut m0) = (y, m);
                for _ in 0..2 {
                    let (py, pm) = prev_month(y0, m0);
                    y0 = py;
                    m0 = pm;
                }
                let start = NaiveDate::from_ymd_opt(y0, m0, 1).unwrap();
                let end = NaiveDate::from_ymd_opt(y, m, last_day_of_month(y, m)).unwrap();
                Some((start, end))
            }
            Self::YearToDate => {
                let start = NaiveDate::from_ymd_opt(y, 1, 1).unwrap();
                Some((start, today))
            }
            Self::ThisYear => {
                let start = NaiveDate::from_ymd_opt(y, 1, 1).unwrap();
                let end = NaiveDate::from_ymd_opt(y, 12, 31).unwrap();
                Some((start, end))
            }
            Self::LastYear => {
                let start = NaiveDate::from_ymd_opt(y - 1, 1, 1).unwrap();
                let end = NaiveDate::from_ymd_opt(y - 1, 12, 31).unwrap();
                Some((start, end))
            }
            Self::Custom => None,
        }
    }
}

impl fmt::Display for PeriodPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ThisMonth => "this_month",
            Self::LastMonth => "last_month",
            Self::Last3Months => "last_3_months",
            Self::YearToDate => "ytd",
            Self::ThisYear => "this_year",
            Self::LastYear => "last_year",
            Self::Custom => "custom",
        })
    }
}

/// Filter applied to a report query.
#[derive(Debug, Clone)]
pub struct ReportFilter {
    pub preset: PeriodPreset,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub account_id: Option<i32>,
    pub payment_method: Option<PaymentMethod>,
}

impl ReportFilter {
    /// Default filter: current month, all accounts, all payment methods.
    pub fn this_month() -> Self {
        let today = Local::now().date_naive();
        let (start, end) = PeriodPreset::ThisMonth.resolve(today).unwrap();
        Self {
            preset: PeriodPreset::ThisMonth,
            start,
            end,
            account_id: None,
            payment_method: None,
        }
    }

    /// Apply a preset, updating `start`/`end` accordingly.
    /// `Custom` leaves the current range untouched.
    pub fn apply_preset(&mut self, preset: PeriodPreset, today: NaiveDate) {
        self.preset = preset;
        if let Some((s, e)) = preset.resolve(today) {
            self.start = s;
            self.end = e;
        }
    }
}

/// Aggregated amount per category for a given transaction type.
#[derive(Debug, Clone)]
pub struct CategoryAggregate {
    pub category_id: i32,
    pub total: Decimal,
    pub count: u32,
}

/// Aggregated totals for one calendar month.
#[derive(Debug, Clone, Copy)]
pub struct MonthlyAggregate {
    pub year: i32,
    pub month: u32,
    pub income: Decimal,
    pub expense: Decimal,
}

impl MonthlyAggregate {
    pub fn net(&self) -> Decimal {
        self.income - self.expense
    }
}

/// Build a complete list of (year, month) buckets covering the inclusive date range
/// [start, end], sorted ascending. Ensures months with zero activity still show up
/// as zero-bars in the trend view.
pub fn month_buckets(start: NaiveDate, end: NaiveDate) -> Vec<(i32, u32)> {
    let mut out = Vec::new();
    let (mut y, mut m) = (start.year(), start.month());
    loop {
        out.push((y, m));
        if (y, m) == (end.year(), end.month()) {
            break;
        }
        let (ny, nm) = next_month(y, m);
        y = ny;
        m = nm;
        // Safety net — if end is earlier than start, don't loop forever.
        if out.len() > 120 {
            break;
        }
    }
    out
}
