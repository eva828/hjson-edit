use hjson_edit::ast::QuoteStyle;

#[test]
fn encode_double_quoted_escapes_backslash() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.encode_body("path\\to\\file"),
        "path\\\\to\\\\file"
    );
}

#[test]
fn encode_double_quoted_escapes_quote() {
    assert_eq!(
        QuoteStyle::DoubleQuoted.encode_body("hello\"world"),
        "hello\\\"world"
    );
}

#[test]
fn encode_double_quoted_no_special_chars_is_identity() {
    assert_eq!(QuoteStyle::DoubleQuoted.encode_body("hello"), "hello");
}

#[test]
fn encode_block_is_identity() {
    assert_eq!(
        QuoteStyle::Block.encode_body("hello\nworld"),
        "hello\nworld"
    );
}

#[test]
fn encode_unquoted_is_identity() {
    assert_eq!(QuoteStyle::Unquoted.encode_body("hello"), "hello");
}
