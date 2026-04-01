//! High-level API tests - unified and simplified
//!
//! Tests that exercise the actual AST operations are marked `#[ignore]` while the
//! implementation is being rebuilt. `invalid_path_errors` is the only test that
//! runs unconditionally because it exercises path *parsing* (which is already
//! implemented), not the AST editing operations.

use hjson_edit::{HjsonDocument, HjsonDocumentObjectEdit, HjsonValue};

// Helper function to make assertions easier
fn expect_value(result: Result<HjsonValue, hjson_edit::HjsonEditError>, expected: HjsonValue) {
    assert_eq!(result.unwrap(), expected);
}

fn expect_error(result: Result<HjsonValue, hjson_edit::HjsonEditError>) {
    assert!(result.is_err());
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn get_and_set_simple() {
    let doc = HjsonDocument::parse(r#"{"name": "test", "value": 42}"#).unwrap();

    expect_value(doc.get("name"), HjsonValue::String("test".to_string()));
    expect_value(doc.get("value"), HjsonValue::Integer(42));

    let updated = doc.set("name", "updated").unwrap();
    expect_value(
        updated.get("name"),
        HjsonValue::String("updated".to_string()),
    );
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn set_nested_path() {
    let doc = HjsonDocument::parse(r#"{"a": {"b": {"c": "value"}}}"#).unwrap();

    let updated = doc.set("a.b.c", "new_value").unwrap();
    expect_value(
        updated.get("a.b.c"),
        HjsonValue::String("new_value".to_string()),
    );
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn set_creates_missing_parents() {
    let doc = HjsonDocument::parse("root:\n{}\n").unwrap();

    let updated = doc.set("root.new.nested", "value").unwrap();
    expect_value(
        updated.get("root.new.nested"),
        HjsonValue::String("value".to_string()),
    );
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn remove_key() {
    let doc = HjsonDocument::parse(r#"{"keep": "a", "remove": "b"}"#).unwrap();

    let updated = doc.remove("remove").unwrap();
    expect_value(updated.get("keep"), HjsonValue::String("a".to_string()));
    expect_error(updated.get("remove"));
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn remove_nested_key() {
    let doc = HjsonDocument::parse(r#"{"parent": {"keep": "a", "remove": "b"}}"#).unwrap();

    let updated = doc.remove("parent.remove").unwrap();
    expect_value(
        updated.get("parent.keep"),
        HjsonValue::String("a".to_string()),
    );
    expect_error(updated.get("parent.remove"));
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn array_push() {
    let doc = HjsonDocument::parse(r#"{"items": [1, 2]}"#).unwrap();

    // Use set with array push syntax "[-]"
    let updated = doc.set("items[-]", 3).unwrap();
    expect_value(updated.get("items[2]"), HjsonValue::Integer(3));
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn array_insert_at() {
    let doc = HjsonDocument::parse(r#"{"items": [1, 3]}"#).unwrap();

    // Use set with array insert syntax "[+n]"
    let updated = doc.set("items[+1]", 2).unwrap();
    expect_value(updated.get("items[0]"), HjsonValue::Integer(1));
    expect_value(updated.get("items[1]"), HjsonValue::Integer(2));
    expect_value(updated.get("items[2]"), HjsonValue::Integer(3));
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn array_remove_at() {
    let doc = HjsonDocument::parse(r#"{"items": [1, 2, 3]}"#).unwrap();

    // Use remove with array index
    let updated = doc.remove("items[1]").unwrap();
    expect_value(updated.get("items[0]"), HjsonValue::Integer(1));
    expect_value(updated.get("items[1]"), HjsonValue::Integer(3));
    expect_error(updated.get("items[2]"));
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn array_in_object_path() {
    let doc = HjsonDocument::parse(r#"{"items": [{"name": "first"}]}"#).unwrap();

    let updated = doc.set("items[0].name", "updated").unwrap();
    expect_value(
        updated.get("items[0].name"),
        HjsonValue::String("updated".to_string()),
    );
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn set_preserves_parent_newline_kind_for_inserted_entry() {
    let input = "{\n  section: {\r\n    first: 1\r\n  }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();

    let updated = doc.set("section.second", 2).unwrap();
    let output = updated.to_string();

    assert!(output.contains("section: {\r\n    first: 1\r\n    second: 2\r\n  }"));
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn push_preserves_parent_newline_kind_for_inserted_entry() {
    let input = "{\n  items: [\r\n    1\r\n    2\r\n  ]\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();

    // Use push syntax
    let updated = doc.set("items[-]", 3).unwrap();
    let output = updated.to_string();

    assert!(output.contains("items: [\r\n    1\r\n    2\r\n    3\r\n  ]"));
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn set_existing_key_preserves_comments() {
    let input = "{\n    // comment\n    name: app\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("name", "api").unwrap();
    let output = updated.to_string();
    assert!(
        output.contains("// comment"),
        "comment should be preserved after set, got: {:?}",
        output
    );
    expect_value(
        updated.get("name"),
        HjsonValue::String("api".to_string()),
    );
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn set_nested_existing_key_preserves_comments() {
    let input = "{\n    // outer comment\n    outer: {\n        // inner comment\n        inner: old\n    }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("outer.inner", "new").unwrap();
    let output = updated.to_string();
    assert!(
        output.contains("// outer comment"),
        "outer comment should be preserved after nested set, got: {:?}",
        output
    );
    assert!(
        output.contains("// inner comment"),
        "inner comment should be preserved after nested set, got: {:?}",
        output
    );
    expect_value(
        updated.get("outer.inner"),
        HjsonValue::String("new".to_string()),
    );
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn set_new_key_in_existing_nested_object_preserves_comments() {
    let input = "{\n    // comment\n    outer: {\n        existing: value\n    }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("outer.new_key", "hello").unwrap();
    let output = updated.to_string();
    assert!(
        output.contains("// comment"),
        "comment should be preserved when adding new nested key, got: {:?}",
        output
    );
    expect_value(
        updated.get("outer.new_key"),
        HjsonValue::String("hello".to_string()),
    );
    expect_value(
        updated.get("outer.existing"),
        HjsonValue::String("value".to_string()),
    );
}

#[test]
fn invalid_path_errors() {
    let doc = HjsonDocument::parse("items: [1, 2]\n").unwrap();

    // Invalid array index should return an InvalidPath error from path *parsing*.
    assert!(doc.clone().remove("items[abc]").is_err());
    assert!(doc.clone().remove("items[1x]").is_err());
}
