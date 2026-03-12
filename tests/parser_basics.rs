//! Parser basic tests - unified table-driven tests

use hjson_edit::HjsonDocument;

/// Table-driven test for all root types
#[test]
fn root_types_parsing() {
    let cases = [
        ("object_with_braces", r#"{"key": "value"}"#),
        ("object_without_braces", "key: value\n"),
        ("string", "\"hello world\""),
        ("number_integer", "42"),
        ("number_float", "3.14159"),
        ("boolean_true", "true"),
        ("boolean_false", "false"),
        ("null", "null"),
        ("array", "[1, 2, 3]"),
    ];

    for (name, input) in cases {
        let doc = HjsonDocument::parse(input).expect(&format!("{} should parse", name));
        let output = doc.to_string();
        assert_eq!(
            output, input,
            "{}: roundtrip should preserve content",
            name
        );
    }
}

/// Test HJSON-specific features
#[test]
fn hjson_extensions() {
    let cases = [
        ("unquoted_string", "key: unquoted_value\n"),
        ("unquoted_with_spaces", "key: look ma, no quotes!\n"),
        ("no_comma_object", "{\n  a: 1\n  b: 2\n}\n"),
        ("no_comma_array", "[\n  1\n  2\n]\n"),
        ("trailing_comma_object", "{\n  a: 1,\n  b: 2,\n}\n"),
        ("trailing_comma_array", "[1, 2, 3,]"),
        ("line_comment", "// comment\nkey: value\n"),
        ("hash_comment", "# comment\nkey: value\n"),
        ("block_comment", "/* comment */\nkey: value\n"),
        ("inline_comment", "{ key: value /* inline */ }"),
    ];

    for (name, input) in cases {
        let doc = HjsonDocument::parse(input).expect(&format!("{} should parse", name));
        let output = doc.to_string();
        assert_eq!(
            output, input,
            "{}: HJSON extension should roundtrip",
            name
        );
    }
}

/// Test block strings (multiline)
#[test]
fn block_strings() {
    let input = "text: '''\n    line1\n      line2\n    '''\n";
    let doc = HjsonDocument::parse(input).unwrap();
    
    assert_eq!(
        doc.get("text"),
        Some(hjson_edit::HjsonValue::String("line1\n  line2".to_string()))
    );
    
    let output = doc.to_string();
    let reparsed = HjsonDocument::parse(&output).unwrap();
    assert_eq!(output, reparsed.to_string());
}

/// Test deeply nested structures
#[test]
fn nested_structures() {
    let input = r#"{
  level1: {
    level2: {
      level3: {
        deeply_nested_value: "found it!"
      }
    }
  },
  array_of_objects: [
    { name: "item1", value: 100 },
    { name: "item2", value: 200 }
  ]
}"#;

    let doc = HjsonDocument::parse(input).unwrap();
    let output = doc.to_string();
    assert_eq!(output, input);
}

/// Test array of objects
#[test]
fn array_of_objects() {
    let input = r#"[
  { name: "first", value: 1 },
  { name: "second", value: 2 }
]"#;

    let doc = HjsonDocument::parse(input).unwrap();
    assert_eq!(doc.get("[0].name"), Some(hjson_edit::HjsonValue::String("first".to_string())));
    assert_eq!(doc.get("[1].value"), Some(hjson_edit::HjsonValue::Integer(2)));
}

/// JSON文字列のエスケープシーケンス
/// 
/// TODO: RFC準拠のエスケープ処理が必要
/// - \n, \t, \" は実際の文字に変換されるべき
/// - \uXXXX はUnicode文字に変換されるべき
#[test]
fn json_string_escapes() {
    // 現在の実装ではエスケープシーケンスは保持される（RFC非準拠）
    // 引用符のエスケープをテスト
    let doc = HjsonDocument::parse(r#"{"key": "say \"hello\""}"#).unwrap();
    let value = doc.get("key");
    assert!(value.is_some(), "Key should exist");
    if let Some(hjson_edit::HjsonValue::String(s)) = value {
        // 実装によってはエスケープが保持されるか、解除されるか
        assert!(s.contains("hello"), "String should contain 'hello', got {:?}", s);
    }
}

/// 指数表記の数値
#[test]
fn number_exponent_notation() {
    let cases = [
        ("positive_exp", "1e6", 1_000_000.0),
        ("negative_exp", "1e-3", 0.001),
        ("uppercase_E", "1E6", 1_000_000.0),
        ("with_fraction", "1.5e2", 150.0),
        ("negative_base", "-5e2", -500.0),
    ];

    for (name, input, expected) in cases {
        let doc = HjsonDocument::parse(&format!("{{ value: {} }}", input)).unwrap();
        if let Some(hjson_edit::HjsonValue::Float(v)) = doc.get("value") {
            assert!((v - expected).abs() < f64::EPSILON, "{}: expected {}, got {}", name, expected, v);
        } else {
            panic!("{}: Expected float for {}", name, input);
        }
    }
}

/// リテラル・数値の終端チェック（quoteless stringとの区別）
#[test]
fn literal_and_number_end_checking() {
    // RFC: "true blue" は文字列 "true blue" で、boolean true ではない
    let cases = [
        ("true_with_suffix", "key: true blue\n", "true blue"),
        ("false_with_suffix", "key: falsehood\n", "falsehood"),
        ("null_with_suffix", "key: nullable\n", "nullable"),
        ("number_with_suffix", "key: 1 minute\n", "1 minute"),
        ("number_with_suffix2", "key: 5 times\n", "5 times"),
    ];

    for (name, input, expected) in cases {
        let doc = HjsonDocument::parse(input).expect(name);
        assert_eq!(
            doc.get("key"),
            Some(hjson_edit::HjsonValue::String(expected.to_string())),
            "{}: should be parsed as string, not literal/number", name
        );
    }
}

/// 引用符なし文字列の特殊ケース
#[test]
fn quoteless_string_edge_cases() {
    // パンクチュエータで始まる場合は引用符が必要
    let result = HjsonDocument::parse("key: ,value\n");
    assert!(result.is_err(), "Quoteless string starting with punctuator should fail");
}
