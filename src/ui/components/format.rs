use rust_decimal::Decimal;

/// Parse a BRL-friendly amount string (accepts comma as decimal separator).
/// Returns error if the value is not a positive number.
pub fn parse_positive_amount(input: &str) -> Result<Decimal, String> {
    input
        .trim()
        .replace(',', ".")
        .parse::<Decimal>()
        .map_err(|_| "Invalid amount".to_string())
        .and_then(|v| {
            if v > Decimal::ZERO {
                Ok(v)
            } else {
                Err("Amount must be positive".into())
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
