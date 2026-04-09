use finances_tui::ui::components::format::{format_brl, parse_positive_amount};
use finances_tui::ui::i18n::Locale;
use rust_decimal_macros::dec;

const EN: Locale = Locale::En;
const PT: Locale = Locale::Pt;

// -- format_brl --

#[test]
fn format_brl_zero() {
    assert_eq!(format_brl(dec!(0)), "R$ 0,00");
}

#[test]
fn format_brl_small_value() {
    assert_eq!(format_brl(dec!(0.50)), "R$ 0,50");
}

#[test]
fn format_brl_whole_number() {
    assert_eq!(format_brl(dec!(100)), "R$ 100,00");
}

#[test]
fn format_brl_thousands_separator() {
    assert_eq!(format_brl(dec!(1234.56)), "R$ 1.234,56");
}

#[test]
fn format_brl_millions() {
    assert_eq!(format_brl(dec!(1234567.89)), "R$ 1.234.567,89");
}

#[test]
fn format_brl_negative() {
    assert_eq!(format_brl(dec!(-100)), "-R$ 100,00");
}

#[test]
fn format_brl_negative_with_thousands() {
    assert_eq!(format_brl(dec!(-1234.56)), "-R$ 1.234,56");
}

// -- parse_positive_amount --

#[test]
fn parse_amount_dot_decimal() {
    assert_eq!(parse_positive_amount("10.50", EN).unwrap(), dec!(10.50));
}

#[test]
fn parse_amount_comma_decimal() {
    assert_eq!(parse_positive_amount("10,50", EN).unwrap(), dec!(10.50));
}

#[test]
fn parse_amount_integer() {
    assert_eq!(parse_positive_amount("100", EN).unwrap(), dec!(100));
}

#[test]
fn parse_amount_with_whitespace() {
    assert_eq!(parse_positive_amount("  42,99  ", EN).unwrap(), dec!(42.99));
}

#[test]
fn parse_amount_zero_rejected() {
    assert!(parse_positive_amount("0", EN).is_err());
}

#[test]
fn parse_amount_negative_rejected() {
    assert!(parse_positive_amount("-10", EN).is_err());
}

#[test]
fn parse_amount_garbage_rejected() {
    assert!(parse_positive_amount("abc", EN).is_err());
}

#[test]
fn parse_amount_empty_rejected() {
    assert!(parse_positive_amount("", EN).is_err());
}

#[test]
fn parse_amount_brl_thousands_format() {
    // BRL style: dots as thousands separators, comma as decimal
    assert_eq!(parse_positive_amount("1.234,56", EN).unwrap(), dec!(1234.56));
}

#[test]
fn parse_amount_brl_millions_format() {
    assert_eq!(
        parse_positive_amount("1.234.567,89", EN).unwrap(),
        dec!(1234567.89)
    );
}

#[test]
fn parse_amount_brl_no_decimal() {
    // BRL style with comma but no fractional part
    assert_eq!(parse_positive_amount("1.000,00", EN).unwrap(), dec!(1000.00));
}

// -- parse_positive_amount localized errors --

#[test]
fn parse_amount_error_message_in_portuguese() {
    let err = parse_positive_amount("abc", PT).unwrap_err();
    assert_eq!(err, "Valor inválido");
}

#[test]
fn parse_amount_zero_error_in_portuguese() {
    let err = parse_positive_amount("0", PT).unwrap_err();
    assert_eq!(err, "Valor deve ser positivo");
}
