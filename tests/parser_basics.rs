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
