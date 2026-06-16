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
fn decode_double_quoted_unicode_escapes_preserved() {
    // NOTE: RFC 8259 では \u0041 は 'A' にデコードされるべきだが、
    // 現在の実装では生のテキストとして保持する（ラウンドトリップ保存のため）。
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"\\u0041\""),
        "\\u0041"
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
fn decode_double_quoted_unknown_escapes_preserved() {
    // NOTE: RFC 8259 では \z は不正なエスケープとしてエラーにすべきだが、
    // 現在の実装では生のテキストとして保持する（ラウンドトリップ保存のため）。
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"\\z\""),
        "\\z"
    );
}
