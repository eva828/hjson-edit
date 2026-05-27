use hjson_edit::HjsonDocument;

// ------------------------------------------------------------------
// These tests expect correct parser behavior per RFC.
// They FAIL with the current implementation due to RFC non-compliance.
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
    let result = HjsonDocument::parse("1 2 3");
    assert!(result.is_err());
}

// ------------------------------------------------------------------
// RFC non-compliance: these SHOULD error but currently parse successfully.
// Changing these to assert!(result.is_err()) makes them FAIL.
// ------------------------------------------------------------------

#[test]
fn parser_whitespace_separated_array_items_should_error() {
    // RFC 8259 / HJSON RFC: array items must be separated by commas or newlines.
    let result = HjsonDocument::parse("[1 2 3]");
    assert!(result.is_err());
}

#[test]
fn parser_whitespace_separated_object_members_should_error() {
    // RFC: object members must be separated by commas or newlines.
    let result = HjsonDocument::parse("{a:1 b:2}");
    assert!(result.is_err());
}

#[test]
fn parser_tab_separated_array_items_should_error() {
    let result = HjsonDocument::parse("[1\t2\t3]");
    assert!(result.is_err());
}

#[test]
fn parser_tab_separated_object_members_should_error() {
    let result = HjsonDocument::parse("{a:1\tb:2}");
    assert!(result.is_err());
}

#[test]
fn parser_string_values_without_comma_should_error() {
    // Even quoted strings separated only by whitespace should require separator.
    let result = HjsonDocument::parse("[\"a\" \"b\"]");
    assert!(result.is_err());
}
