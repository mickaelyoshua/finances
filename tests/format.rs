use finances::ui::components::format::{format_brl, parse_positive_amount};
use rust_decimal_macros::dec;

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
    assert_eq!(parse_positive_amount("10.50").unwrap(), dec!(10.50));
}

#[test]
fn parse_amount_comma_decimal() {
    assert_eq!(parse_positive_amount("10,50").unwrap(), dec!(10.50));
}

#[test]
fn parse_amount_integer() {
    assert_eq!(parse_positive_amount("100").unwrap(), dec!(100));
}

#[test]
fn parse_amount_with_whitespace() {
    assert_eq!(parse_positive_amount("  42,99  ").unwrap(), dec!(42.99));
}

#[test]
fn parse_amount_zero_rejected() {
    assert!(parse_positive_amount("0").is_err());
}

#[test]
fn parse_amount_negative_rejected() {
    assert!(parse_positive_amount("-10").is_err());
}

#[test]
fn parse_amount_garbage_rejected() {
    assert!(parse_positive_amount("abc").is_err());
}

#[test]
fn parse_amount_empty_rejected() {
    assert!(parse_positive_amount("").is_err());
}
