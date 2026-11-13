use hjson_edit::{HjsonDocument, HjsonEditError};

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

#[test]
fn invalid_escape_after_a_comment_like_key_is_reported() {
    // キーの `//` は正当。その後ろの再字句解析で見つかった不正エスケープを捨てない
    for text in ["{ a//b: \"\\z\", }", "a//b: \"\\z\""] {
        let error = HjsonDocument::parse(text).unwrap_err();

        assert!(
            matches!(error, HjsonEditError::LexError { .. }),
            "text: {text:?}, error: {error:?}"
        );
        assert!(
            error.to_string().contains("Invalid escape sequence"),
            "text: {text:?}, error: {error}"
        );
    }
}

#[test]
fn unterminated_string_after_a_comment_like_key_is_reported() {
    let error = HjsonDocument::parse("{ a//b: \"x }").unwrap_err();

    assert!(
        error.to_string().contains("Unterminated string"),
        "error: {error}"
    );
}

#[test]
fn comment_like_keys_still_parse() {
    for text in ["{ a//b: 1 }", "{ a/*b*/c: 1 }", "a//b: 1"] {
        let document = HjsonDocument::parse(text).unwrap();

        assert_eq!(document.to_string(), text);
    }
}

#[test]
fn retry_lex_errors_are_not_reported_twice() {
    let error = HjsonDocument::parse("{ a//b: \"\\z\", c//d: \"\\q\" }")
        .unwrap_err()
        .to_string();

    assert_eq!(
        error.matches("Invalid escape sequence").count(),
        2,
        "error: {error}"
    );
}

#[test]
fn text_swallowed_by_a_comment_like_key_is_not_lexed_twice() {
    // 初回の字句解析では `//` 以降がコメントになるが、キーの走査で読み直される。
    // 読み直した領域の文字列はブロック文字列の中身なので、不正なエスケープではない
    let input = "{a: -3.5, key: 1e2, a//b: '''\n  \"\\z\"\n  '''}";
    let document = HjsonDocument::parse(input).unwrap();

    assert_eq!(document.to_string(), input);
}

#[test]
fn relexed_errors_are_reported_once() {
    let error = HjsonDocument::parse("{ a//b: \"\\z\", }").unwrap_err();

    assert!(matches!(error, HjsonEditError::LexError { .. }));
    assert_eq!(
        error.to_string().matches("Invalid escape sequence").count(),
        1,
        "error: {error}"
    );
}

#[test]
fn adding_a_member_after_an_unterminated_comment_names_the_missing_value() {
    use hjson_edit::high_level::ops::object::HjsonDocumentEdit;
    use hjson_edit::high_level::path::HjsonObjectPath;
    use hjson_edit::high_level::verify::ValueEdit;
    use hjson_edit::HjsonValue;

    // `/*c` は文書の末尾までを飲み込むため、足したメンバーは値として現れない
    let original = HjsonDocument::parse("x: 5/*c").unwrap();
    let path = HjsonObjectPath::parse("a").unwrap();
    let updated = original
        .clone()
        .verify_edits(false)
        .set(path.clone(), 7)
        .unwrap();
    let error = updated
        .verify_value_edits(&original, &[ValueEdit::Set(path, HjsonValue::Integer(7))])
        .unwrap_err();

    assert!(
        matches!(error, HjsonEditError::InvalidEdit { ref target, .. } if target == "a"),
        "{error:?}"
    );
    assert!(error.to_string().contains("found no value"), "{error}");
}
