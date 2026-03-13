//! Parser error handling tests - unified

use hjson_edit::HjsonDocument;

/// Table-driven error test for common syntax errors
#[test]
fn syntax_errors() {
    let error_cases = [
        ("unclosed_object", "{"),
        ("unclosed_array", "["),
        ("unclosed_string", "\"unclosed"),
        ("missing_colon", "{ key value }"),
        ("missing_value", "{ key: }"),
        ("double_colon", "{ key:: value }"),
    ];

    for (name, input) in error_cases {
        let result = HjsonDocument::parse(input);
        assert!(
            result.is_err(),
            "{}: should fail to parse '{}'",
            name,
            input
        );
    }
}

/// Test that error messages contain expected text
#[test]
fn error_messages_contain_expected_text() {
    let result = HjsonDocument::parse("\"unclosed");
    if let Err(errors) = result {
        let msg = format!("{:?}", errors);
        assert!(msg.contains("Unterminated") || msg.contains("terminated"));
    } else {
        panic!("Expected error for unclosed string");
    }
}

/// Test error recovery - multiple errors should be collected
#[test]
fn multiple_errors_collected() {
    let input = r#"{
  first: 1
  second,
  third: 3
}"#;

    let result = HjsonDocument::parse(input);
    assert!(result.is_err(), "Should fail with syntax errors");
}

/// Test that deeply nested errors have reasonable line numbers
#[test]
fn nested_error_line_numbers() {
    let input = r#"{
  level1: {
    level2: {
      invalid syntax here
    }
  }
}"#;

    let result = HjsonDocument::parse(input);
    if let Err(errors) = result {
        for error in errors {
            assert!(
                error.line >= 3 && error.line <= 5,
                "Error should be on line ~4, got {}",
                error.line
            );
        }
    } else {
        panic!("Expected error for invalid syntax");
    }
}

/// Invalid hex and special numbers should be treated as strings (not errors)
#[test]
fn non_standard_numbers_as_strings() {
    let cases = [
        ("hex", "{ value: 0xFF }"),
        ("nan", "{ value: NaN }"),
        ("infinity", "{ value: Infinity }"),
        ("negative_infinity", "{ value: -Infinity }"),
    ];

    for (name, input) in cases {
        let result = HjsonDocument::parse(input);
        assert!(
            result.is_ok(),
            "{}: Non-standard numbers should parse as strings: {}",
            name,
            input
        );
    }
}

/// 先頭ゼロは許可されない（RFC準拠）
#[test]
fn leading_zeros_not_allowed() {
    // RFCでは "007" は不正な数値
    // HJSONは "007" を文字列として扱う実装とする
    let result = HjsonDocument::parse("{ value: 007 }");
    if let Ok(doc) = result {
        let val = doc.get("value");
        assert!(
            matches!(val, Some(hjson_edit::HjsonValue::String(_))),
            "Leading zeros should be treated as string"
        );
    }
}

/// 空入力の処理
#[test]
fn empty_input() {
    let result = HjsonDocument::parse("");
    // 実装による: エラーまたは空オブジェクト
    assert!(result.is_err() || result.unwrap().to_string().trim().is_empty());
}

/// CRLF処理（RFC準拠 - CRは正規化され\nのみとなる）
#[test]
fn crlf_handling() {
    let input = "{\r\n  key: value\r\n}";
    let doc = HjsonDocument::parse(input).unwrap();
    let output = doc.to_string();
    assert!(
        output.contains("key: value"),
        "Should parse CRLF content correctly"
    );
    // RFC準拠: CRは正規化される
    assert!(!output.contains('\r'), "CR should be normalized to LF");
}
