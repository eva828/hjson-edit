use hjson_edit::high_level::{
    ops::object::HjsonDocumentObjectEdit, path::HjsonObjectPath, value::HjsonValue, HjsonDocument,
};

// quoteless 値の中に現れる `"` / `'''` は値の一部であり、文字列の開始ではない。
// 文字列として開始してよいのは値・キーの先頭だけである。

fn string_value(input: &str, path: &str) -> String {
    let doc = HjsonDocument::parse(input).unwrap();
    match doc.get(HjsonObjectPath::parse(path).unwrap()).unwrap() {
        HjsonValue::String(s) => s,
        other => panic!("expected String, got {other:?}"),
    }
}

#[test]
fn quote_after_punctuator_in_quoteless_value_is_part_of_value() {
    assert_eq!(
        string_value("a: x{0}\"y\n", "a"),
        "x{0}\"y",
        "区切り文字の直後の \" は値の一部"
    );
}

#[test]
fn quote_after_space_in_quoteless_value_is_part_of_value() {
    assert_eq!(string_value("a: ab \"cd\n", "a"), "ab \"cd");
}

#[test]
fn quote_after_colon_in_quoteless_value_is_part_of_value() {
    assert_eq!(string_value("a: x:\"y\n", "a"), "x:\"y");
}

#[test]
fn quote_pair_in_quoteless_value_is_part_of_value() {
    assert_eq!(string_value("a: x\"{0}\"y\n", "a"), "x\"{0}\"y");
}

#[test]
fn quote_at_end_of_quoteless_value_is_part_of_value() {
    assert_eq!(string_value("a: xy\"\n", "a"), "xy\"");
}

#[test]
fn comment_like_text_after_quote_is_part_of_quoteless_value() {
    assert_eq!(string_value("a: xy\" // c\n", "a"), "xy\" // c");
}

#[test]
fn multibyte_value_with_quotes_is_preserved() {
    assert_eq!(
        string_value("a: 権限が\"{0}\"足りない\n", "a"),
        "権限が\"{0}\"足りない"
    );
}

#[test]
fn bracketed_text_with_trailing_quote_is_preserved() {
    assert_eq!(
        string_value("a: 他に{0}件 [c/A49DAE:(表示)]\"\n", "a"),
        "他に{0}件 [c/A49DAE:(表示)]\""
    );
}

#[test]
fn unterminated_string_at_value_start_is_rejected() {
    let err = HjsonDocument::parse("a: \"foo").unwrap_err().to_string();
    assert!(err.contains("Unterminated string literal"), "got: {err}");
}

#[test]
fn unterminated_block_string_at_value_start_is_rejected() {
    let err = HjsonDocument::parse("a: '''foo").unwrap_err().to_string();
    assert!(err.contains("Unterminated block string"), "got: {err}");
}

#[test]
fn unterminated_string_at_key_start_is_rejected() {
    let err = HjsonDocument::parse("\"a: 1").unwrap_err().to_string();
    assert!(err.contains("Unterminated string literal"), "got: {err}");
}

#[test]
fn unterminated_string_in_array_element_is_rejected() {
    let err = HjsonDocument::parse("[1, \"x").unwrap_err().to_string();
    assert!(err.contains("Unterminated string literal"), "got: {err}");
}

#[test]
fn terminated_string_value_still_parses_as_string() {
    assert_eq!(string_value("a: \"foo\"\n", "a"), "foo");
}

#[test]
fn terminated_string_with_escaped_quote_still_parses() {
    assert_eq!(string_value("a: \"a\\\"b\"\n", "a"), "a\"b");
}

#[test]
fn multiline_block_string_value_still_parses() {
    assert_eq!(string_value("a: '''\nx\n'''\n", "a"), "x");
}

#[test]
fn invalid_escape_in_terminated_string_is_rejected() {
    assert!(HjsonDocument::parse("a: \"a\\qb\"").is_err());
}

#[test]
fn quoted_key_is_not_treated_as_quoteless() {
    let doc = HjsonDocument::parse("{\"a\": 1}\n").unwrap();
    assert!(matches!(
        doc.get(HjsonObjectPath::parse("a").unwrap()).unwrap(),
        HjsonValue::Integer(1)
    ));
}
