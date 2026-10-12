use hjson_edit::ast::QuoteStyle;

#[test]
fn decode_double_quoted_simple() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\"", 0),
        "hello"
    );
}

#[test]
fn decode_double_quoted_with_escaped_quote() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\\"world\"", 0),
        "hello\"world"
    );
}

#[test]
fn decode_double_quoted_with_escaped_backslash() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"path\\\\to\\\\file\"", 0),
        "path\\to\\file"
    );
}

#[test]
fn decode_double_quoted_with_escaped_solidus() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\/world\"", 0),
        "hello/world"
    );
}

#[test]
fn decode_double_quoted_with_escaped_backspace() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\bworld\"", 0),
        "hello\u{0008}world"
    );
}

#[test]
fn decode_double_quoted_with_escaped_formfeed() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\fworld\"", 0),
        "hello\u{000C}world"
    );
}

#[test]
fn decode_double_quoted_with_escaped_newline() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\nworld\"", 0),
        "hello\nworld"
    );
}

#[test]
fn decode_double_quoted_with_escaped_carriage_return() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\rworld\"", 0),
        "hello\rworld"
    );
}

#[test]
fn decode_double_quoted_with_escaped_tab() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"hello\\tworld\"", 0),
        "hello\tworld"
    );
}

#[test]
fn decode_double_quoted_unicode_escapes_are_decoded() {
    // 基準実装（hjson-cs）と同じく \uXXXX をデコードする
    assert_eq!(QuoteStyle::DoubleQuoted.decode_body("\"\\u0041\"", 0), "A");
    // サロゲート対は 1 文字にまとめる
    assert_eq!(
        QuoteStyle::DoubleQuoted.decode_body("\"\\uD83D\\uDE00\"", 0),
        "\u{1F600}"
    );
}

#[test]
fn decode_block_simple() {
    assert_eq!(QuoteStyle::Block.decode_body("'''hello'''", 0), "hello");
}

#[test]
fn decode_block_empty() {
    assert_eq!(QuoteStyle::Block.decode_body("''''''", 0), "");
}

#[test]
fn decode_block_strips_indent_columns() {
    // 基準実装（hjson-cs）は `'''` のカラム分だけインデントを取り除く
    let raw = "'''\n  line1\n  line2\n'''";
    assert_eq!(QuoteStyle::Block.decode_body(raw, 0), "  line1\n  line2");
    assert_eq!(QuoteStyle::Block.decode_body(raw, 2), "line1\nline2");
}

#[test]
fn decode_block_crlf_normalized() {
    assert_eq!(
        QuoteStyle::Block.decode_body("'''hello\r\nworld'''", 0),
        "hello\nworld"
    );
}

#[test]
fn decode_block_cr_normalized() {
    assert_eq!(
        QuoteStyle::Block.decode_body("'''hello\rworld'''", 0),
        "hello\nworld"
    );
}

#[test]
fn decode_unquoted_returns_raw() {
    assert_eq!(QuoteStyle::Unquoted.decode_body("hello", 0), "hello");
}

#[test]
fn decode_double_quoted_unknown_escapes_preserved() {
    // NOTE: RFC 8259 では \z は不正なエスケープとしてエラーにすべきだが、
    // 現在の実装では生のテキストとして保持する（ラウンドトリップ保存のため）。
    assert_eq!(QuoteStyle::DoubleQuoted.decode_body("\"\\z\"", 0), "\\z");
}
