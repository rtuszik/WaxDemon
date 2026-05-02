use waxdemon_core::parse_currency;

#[test]
fn none_input_returns_none() {
    assert_eq!(parse_currency(None), None);
}

#[test]
fn empty_string_returns_none() {
    assert_eq!(parse_currency(Some("")), None);
}

#[test]
fn plain_integer() {
    assert_eq!(parse_currency(Some("42")), Some(42.0));
}

#[test]
fn plain_float() {
    assert_eq!(parse_currency(Some("12.34")), Some(12.34));
}

#[test]
fn dollar_prefix_with_decimals() {
    assert_eq!(parse_currency(Some("$1234.56")), Some(1234.56));
}

#[test]
fn dollar_prefix_with_thousands_separator() {
    // `$1,234.56`: strip `$` and `,` in one pass -> `1234.56`, then keep last dot -> `1234.56`.
    assert_eq!(parse_currency(Some("$1,234.56")), Some(1234.56));
}

#[test]
fn euro_thousands_dot_decimal_comma() {
    // `€1.234,56`: stripping `,` and `€` together yields `1.23456`. The parser cannot
    // distinguish EU-style decimal commas from US-style thousands commas, so it picks
    // the US convention. Documented quirk, not a bug — kept for parity with stored data.
    assert_eq!(parse_currency(Some("€1.234,56")), Some(1.23456));
}

#[test]
fn yen_no_decimals() {
    assert_eq!(parse_currency(Some("¥1234")), Some(1234.0));
}

#[test]
fn pound_with_decimals() {
    assert_eq!(parse_currency(Some("£99.99")), Some(99.99));
}

#[test]
fn thousands_separator_alone() {
    // `1,234` -> after strip `[$,€£¥]` -> `1234` -> Some(1234.0)
    assert_eq!(parse_currency(Some("1,234")), Some(1234.0));
}

#[test]
fn multiple_dots_keep_last_as_decimal() {
    // `1.234.567` -> only last `.` survives -> `1234.567`
    assert_eq!(parse_currency(Some("1.234.567")), Some(1234.567));
}

#[test]
fn garbage_returns_none() {
    assert_eq!(parse_currency(Some("not a number")), None);
}
