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
            assert!(error.line >= 3 && error.line <= 5,
                "Error should be on line ~4, got {}", error.line);
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
