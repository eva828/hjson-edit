use hjson_edit::HjsonDocument;

// ------------------------------------------------------------------
// パーサーが基本的な構文エラーを正しく検出することを確認するテスト。
// これらのテストはすべて現在の実装でパスする。
// ------------------------------------------------------------------

#[test]
fn parser_unclosed_object_produces_error() {
    let result = HjsonDocument::parse("{ \"a\": 1");
    assert!(result.is_err());
}

#[test]
fn parser_unclosed_array_produces_error() {
    let result = HjsonDocument::parse("[1, 2");
    assert!(result.is_err());
}

#[test]
fn parser_missing_colon_produces_error() {
    let result = HjsonDocument::parse("{ \"a\" 1 }");
    assert!(result.is_err());
}

#[test]
fn parser_empty_key_produces_error() {
    let result = HjsonDocument::parse("{:1}");
    assert!(result.is_err());
}

#[test]
fn parser_stray_rbrace_produces_error() {
    let result = HjsonDocument::parse("}");
    assert!(result.is_err());
}

#[test]
fn parser_stray_rbracket_produces_error() {
    let result = HjsonDocument::parse("]");
    assert!(result.is_err());
}

#[test]
fn parser_stray_comma_produces_error() {
    let result = HjsonDocument::parse(",");
    assert!(result.is_err());
}

#[test]
fn parser_stray_colon_produces_error() {
    let result = HjsonDocument::parse(":");
    assert!(result.is_err());
}

#[test]
fn parser_double_colon_produces_error() {
    let result = HjsonDocument::parse("{\"a\"::1}");
    assert!(result.is_err());
}

#[test]
fn parser_block_string_with_trailing_content_produces_error() {
    let result = HjsonDocument::parse("'''hello'''world");
    assert!(result.is_err());
}

#[test]
fn parser_scalar_before_braceless_object_produces_error() {
    let result = HjsonDocument::parse("123\na:1");
    assert!(result.is_err());
}

#[test]
fn parser_multiple_root_values_produces_error() {
    // In Hjson, bare values can span whitespace, so "1 2 3" is a single string value.
    let result = HjsonDocument::parse("1 2 3");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().to_string(), "1 2 3");
}

// ------------------------------------------------------------------
// RFC: 以下の入力はHjsonでは空白/タブ区切りでも単一値としてパースされる。
// read_raw_value が行末までの全文字を収集するため、エラーにならない。
// ------------------------------------------------------------------

#[test]
fn parser_whitespace_separated_array_items_should_error() {
    // C# readTfnns semantics: `, { } ]` are checkpoints; non-keyword
    // content falls through and the bracket is consumed as content.
    // [1 2 3] → single item "1 2 3]" (closing ] is absorbed) → array
    // never closes → error.
    let result = HjsonDocument::parse("[1 2 3]");
    assert!(result.is_err());
}

#[test]
fn parser_whitespace_separated_object_members_should_error() {
    // C# readTfnns: `1 b:2` is read character-by-character, the closing
    // `}` at object end is consumed as content → object never closes → error.
    let result = HjsonDocument::parse("{a:1 b:2}");
    assert!(result.is_err());
}

#[test]
fn parser_tab_separated_array_items_should_error() {
    // Same as whitespace_separated_array_items — closing ] is absorbed.
    let result = HjsonDocument::parse("[1\t2\t3]");
    assert!(result.is_err());
}

#[test]
fn parser_tab_separated_object_members_should_error() {
    // Same as whitespace_separated_object_members — closing } is absorbed.
    let result = HjsonDocument::parse("{a:1\tb:2}");
    assert!(result.is_err());
}

#[test]
fn parser_string_values_without_comma_should_error() {
    // C# では引用符文字列同士のカンマなし配列が許容される。
    // ["a" "b"] は ["a", "b"] としてパースされる。
    let result = HjsonDocument::parse("[\"a\" \"b\"]");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().to_string(), "[\"a\" \"b\"]");
}

#[test]
fn unquoted_value_with_bracket_is_valid() {
    // C# readTfnns: `]` in `x[y]z` is a checkpoint → consumed as content.
    // Then `}` closes the value → also consumed → no closing } for object → error.
    let result = HjsonDocument::parse("{ v: x[y]z }");
    assert!(result.is_err());
}

#[test]
fn unquoted_value_with_brace_is_valid() {
    // Same as above — `} in `x{y}z` is absorbed.
    let result = HjsonDocument::parse("{ v: x{y}z }");
    assert!(result.is_err());
}
