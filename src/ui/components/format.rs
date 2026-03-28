use rust_decimal::Decimal;

use crate::ui::i18n::{Locale, t};

/// Parse a BRL-friendly amount string.
/// Accepts both `1234,56` (comma decimal) and `1.234,56` (dot thousands + comma decimal).
/// If the input contains a comma, any dots are treated as thousands separators and stripped.
/// Returns error if the value is not a positive number.
pub fn parse_positive_amount(input: &str, locale: Locale) -> Result<Decimal, String> {
    let trimmed = input.trim();
    let normalized = if trimmed.contains(',') {
        // BRL-style: dots are thousands separators, comma is decimal
        trimmed.replace('.', "").replace(',', ".")
    } else {
        trimmed.to_string()
    };
    normalized
        .parse::<Decimal>()
        .map_err(|_| t(locale, "err.invalid_amount").to_string())
        .and_then(|v| {
            if v > Decimal::ZERO {
                Ok(v)
            } else {
                Err(t(locale, "err.amount_positive").to_string())
            }
        })
}

/// Format a Decimal value as Brazilian Real (BRL).
/// Uses Brazilian convention: dot as thousands separator, comma as decimal separator.
/// Examples: R$ 1.234,56  |  R$ 0,50  |  -R$ 100,00
pub fn format_brl(value: Decimal) -> String {
    let is_negative = value.is_sign_negative();
    let abs = value.abs();
    let s = format!("{:.2}", abs);
    let (int_part, dec_part) = s.split_once('.').unwrap_or((&s, "00"));

    // Add thousand separators (dots)
    let chars: Vec<char> = int_part.chars().collect();
    let mut with_sep = String::with_capacity(int_part.len() + int_part.len() / 3);
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i).is_multiple_of(3) {
            with_sep.push('.');
        }
        with_sep.push(*c);
    }

    if is_negative {
        format!("-R$ {},{}", with_sep, dec_part)
    } else {
        format!("R$ {},{}", with_sep, dec_part)
    }
}
