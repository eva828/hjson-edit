use hjson_edit::HjsonDocument;

// ------------------------------------------------------------------
// レクサーが正しくエラー/成功を返すことを確認するテスト。
// 注意: いくつかのテストはRFCに完全準拠していない場合がある（コメント参照）。
// ------------------------------------------------------------------

#[test]
fn unterminated_block_comment_is_skipped_with_a_warning() {
    // 基準実装（hjson-cs）は閉じないブロックコメントを読み飛ばす。切断の検出は CLI の役割。
    let document = HjsonDocument::parse("/* hello").unwrap();
    assert_eq!(document.warnings().len(), 1);
    assert!(document.warnings()[0].contains("Unterminated block comment"));
}

#[test]
fn lexer_unterminated_block_string_produces_error() {
    let result = HjsonDocument::parse("'''hello");
    assert!(result.is_err());
}

#[test]
fn lexer_unterminated_double_quoted_string_produces_error() {
    let result = HjsonDocument::parse("\"hello");
    assert!(result.is_err());
}

#[test]
fn lexer_newline_in_double_quoted_string_produces_error() {
    let result = HjsonDocument::parse("\"hello\nworld\"");
    assert!(result.is_err());
}

#[test]
fn lexer_empty_double_quoted_string_is_valid() {
    // RFC/JSON: "" is a valid empty string.
    let result = HjsonDocument::parse("\"\"");
    assert!(result.is_ok());
}

#[test]
fn lexer_escaped_backslash_at_start_of_string() {
    // RFC: "\\\\" is a valid string containing a single backslash.
    let result = HjsonDocument::parse("\"\\\\\"");
    assert!(result.is_ok());
}

#[test]
fn lexer_invalid_escape_x_at_start_is_rejected() {
    // RFC: \\x is not a valid escape sequence and should error.
    let result = HjsonDocument::parse("\"\\x\"");
    assert!(result.is_err());
}

#[test]
fn lexer_invalid_escape_z_at_start_is_rejected() {
    // RFC: \\z is not a valid escape sequence and should error.
    let result = HjsonDocument::parse("\"\\z\"");
    assert!(result.is_err());
}

#[test]
fn lexer_incomplete_unicode_escape_at_start_is_rejected() {
    // RFC: \\u without valid hex should error.
    let result = HjsonDocument::parse("\"\\u\"");
    assert!(result.is_err());
}

#[test]
fn lexer_invalid_unicode_hex_at_start_is_rejected() {
    // RFC: \\uZZZZ should error.
    let result = HjsonDocument::parse("\"\\uZZZZ\"");
    assert!(result.is_err());
}

#[test]
fn lexer_unexpected_punctuator_in_quoteless_context_produces_error() {
    let result = HjsonDocument::parse("{");
    assert!(result.is_err());
}

#[test]
fn lexer_empty_input_parses_successfully() {
    let result = HjsonDocument::parse("");
    assert!(result.is_ok());
}

#[test]
fn lexer_whitespace_only_input_parses_successfully() {
    let result = HjsonDocument::parse("   ");
    assert!(result.is_ok());
}

#[test]
fn lexer_line_comment_only_input_parses_successfully() {
    let result = HjsonDocument::parse("// hello");
    assert!(result.is_ok());
}

#[test]
fn lexer_hash_comment_only_input_parses_successfully() {
    let result = HjsonDocument::parse("# hello");
    assert!(result.is_ok());
}
