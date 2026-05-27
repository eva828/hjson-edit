use hjson_edit::ast::QuoteStyle;

#[test]
fn decode_double_quoted_simple() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\""),
        "hello"
    );
}

#[test]
fn decode_double_quoted_with_escaped_quote() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\\"world\""),
        "hello\"world"
    );
}

#[test]
fn decode_double_quoted_with_escaped_backslash() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"path\\\\to\\\\file\""),
        "path\\to\\file"
    );
}

#[test]
fn decode_double_quoted_with_escaped_solidus() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\/world\""),
        "hello/world"
    );
}

#[test]
fn decode_double_quoted_with_escaped_backspace() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\bworld\""),
        "hello\u{0008}world"
    );
}

#[test]
fn decode_double_quoted_with_escaped_formfeed() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\fworld\""),
        "hello\u{000C}world"
    );
}

#[test]
fn decode_double_quoted_with_escaped_newline() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\nworld\""),
        "hello\nworld"
    );
}

#[test]
fn decode_double_quoted_with_escaped_carriage_return() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\rworld\""),
        "hello\rworld"
    );
}

#[test]
fn decode_double_quoted_with_escaped_tab() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\tworld\""),
        "hello\tworld"
    );
}

#[test]
fn decode_double_quoted_with_unicode_escape() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"\\u0041\""),
        "A"
    );
}

#[test]
fn decode_double_quoted_with_unicode_emoji() {
    // U+1F600 = 😀 via surrogate pair \uD83D\uDE00
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"\\uD83D\\uDE00\""),
        "😀"
    );
}

#[test]
fn decode_block_simple() {
    assert_eq!(
        QuoteStyle::Block.decode_body("'''hello'''"),
        "hello"
    );
}

#[test]
fn decode_block_empty() {
    assert_eq!(
        QuoteStyle::Block.decode_body("''''''"),
        ""
    );
}

#[test]
fn decode_block_strips_common_indent() {
    let raw = "'''\n  line1\n  line2\n'''";
    assert_eq!(
        QuoteStyle::Block.decode_body(raw),
        "line1\nline2"
    );
}

#[test]
fn decode_block_crlf_normalized() {
    assert_eq!(
        QuoteStyle::Block.decode_body("'''hello\r\nworld'''"),
        "hello\nworld"
    );
}

#[test]
fn decode_block_cr_normalized() {
    assert_eq!(
        QuoteStyle::Block.decode_body("'''hello\rworld'''"),
        "hello\nworld"
    );
}

#[test]
fn decode_unquoted_returns_raw() {
    assert_eq!(
        QuoteStyle::Unquoted.decode_body("hello"),
        "hello"
    );
}

#[test]
fn decode_double_quoted_with_isolated_high_surrogate_returns_raw() {
    let result = QuoteStyle::DoubleQuoted.decode_body("\"\\uD83D\"");
    assert_eq!(result, "\\uD83D");
}

#[test]
fn decode_double_quoted_with_isolated_low_surrogate_returns_raw() {
    let result = QuoteStyle::DoubleQuoted.decode_body("\"\\uDE00\"");
    assert_eq!(result, "\\uDE00");
}

#[test]
fn decode_double_quoted_with_invalid_hex_sequence_returns_raw() {
    let result = QuoteStyle::DoubleQuoted.decode_body("\"\\u123Z\"");
    assert_eq!(result, "\\u123Z");
}

#[test]
fn decode_double_quoted_with_invalid_escape_returns_raw() {
    let result = QuoteStyle::DoubleQuoted.decode_body("\"\\z\"");
    assert_eq!(result, "\\z");
}
