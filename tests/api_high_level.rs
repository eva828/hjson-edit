//! High-level API tests - unified and simplified

use hjson_edit::{CommentPath, CommentStyle, HjsonDocument, HjsonValue, StringStyle};

#[test]
fn get_and_set_simple() {
    let doc = HjsonDocument::parse(r#"{"name": "test", "value": 42}"#).unwrap();
    
    assert_eq!(doc.get("name"), Some(HjsonValue::String("test".to_string())));
    assert_eq!(doc.get("value"), Some(HjsonValue::Integer(42)));
    
    let updated = doc.set("name", "updated").unwrap();
    assert_eq!(updated.get("name"), Some(HjsonValue::String("updated".to_string())));
}

#[test]
fn set_nested_path() {
    let doc = HjsonDocument::parse(r#"{"a": {"b": {"c": "value"}}}"#).unwrap();
    
    let updated = doc.set("a.b.c", "new_value").unwrap();
    assert_eq!(updated.get("a.b.c"), Some(HjsonValue::String("new_value".to_string())));
}

#[test]
fn set_creates_missing_parents() {
    let doc = HjsonDocument::parse("root:\n{}\n").unwrap();
    
    let updated = doc.set("root.new.nested", "value").unwrap();
    assert_eq!(updated.get("root.new.nested"), Some(HjsonValue::String("value".to_string())));
}

#[test]
fn remove_key() {
    let doc = HjsonDocument::parse(r#"{"keep": "a", "remove": "b"}"#).unwrap();
    
    let updated = doc.remove("remove").unwrap();
    assert_eq!(updated.get("keep"), Some(HjsonValue::String("a".to_string())));
    assert_eq!(updated.get("remove"), None);
}

#[test]
fn remove_nested_key() {
    let doc = HjsonDocument::parse(r#"{"parent": {"keep": "a", "remove": "b"}}"#).unwrap();
    
    let updated = doc.remove("parent.remove").unwrap();
    assert_eq!(updated.get("parent.keep"), Some(HjsonValue::String("a".to_string())));
    assert_eq!(updated.get("parent.remove"), None);
}

#[test]
fn array_push() {
    let doc = HjsonDocument::parse(r#"{"items": [1, 2]}"#).unwrap();
    
    let updated = doc.push("items", 3).unwrap();
    assert_eq!(updated.get("items[2]"), Some(HjsonValue::Integer(3)));
}

#[test]
fn array_insert_at() {
    let doc = HjsonDocument::parse(r#"{"items": [1, 3]}"#).unwrap();
    
    let updated = doc.insert_at("items", 1, 2).unwrap();
    assert_eq!(updated.get("items[0]"), Some(HjsonValue::Integer(1)));
    assert_eq!(updated.get("items[1]"), Some(HjsonValue::Integer(2)));
    assert_eq!(updated.get("items[2]"), Some(HjsonValue::Integer(3)));
}

#[test]
fn array_remove_at() {
    let doc = HjsonDocument::parse(r#"{"items": [1, 2, 3]}"#).unwrap();
    
    let updated = doc.remove_at("items", 1).unwrap();
    assert_eq!(updated.get("items[0]"), Some(HjsonValue::Integer(1)));
    assert_eq!(updated.get("items[1]"), Some(HjsonValue::Integer(3)));
    assert_eq!(updated.get("items[2]"), None);
}

#[test]
fn array_in_object_path() {
    let doc = HjsonDocument::parse(r#"{"items": [{"name": "first"}]}"#).unwrap();
    
    let updated = doc.set("items[0].name", "updated").unwrap();
    assert_eq!(updated.get("items[0].name"), Some(HjsonValue::String("updated".to_string())));
}

#[test]
fn comment_set_header() {
    let doc = HjsonDocument::parse("key: value\n").unwrap();
    
    let updated = doc.comment_set(&CommentPath::Header, "Header comment", Some(CommentStyle::Line)).unwrap();
    let output = updated.to_string();
    assert!(output.contains("// Header comment"));
}

#[test]
fn comment_set_key() {
    let doc = HjsonDocument::parse("{\n  key: value\n}").unwrap();
    
    let updated = doc.comment_set(
        &CommentPath::Key { dot_path: "key".into() },
        "Key comment",
        Some(CommentStyle::Line)
    ).unwrap();
    let output = updated.to_string();
    assert!(output.contains("// Key comment"));
}

#[test]
fn comment_set_inline() {
    let doc = HjsonDocument::parse("{\n  key: value\n}").unwrap();
    
    let updated = doc.comment_set(
        &CommentPath::Inline { dot_path: "key".into() },
        "inline",
        Some(CommentStyle::Block)
    ).unwrap();
    let output = updated.to_string();
    assert!(output.contains("/* inline */"));
}

#[test]
fn comment_append() {
    let doc = HjsonDocument::parse("// existing\nkey: value\n").unwrap();
    
    let updated = doc.comment_append(&CommentPath::Header, "appended", Some(CommentStyle::Line)).unwrap();
    let output = updated.to_string();
    assert!(output.contains("// existing"));
    assert!(output.contains("// appended"));
}

#[test]
fn comment_remove() {
    let doc = HjsonDocument::parse("// comment\nkey: value\n").unwrap();
    
    let updated = doc.comment_remove(&CommentPath::Header).unwrap();
    let output = updated.to_string();
    assert!(!output.contains("// comment"));
}

#[test]
fn invalid_path_errors() {
    let doc = HjsonDocument::parse("items: [1, 2]\n").unwrap();
    
    assert!(matches!(
        doc.remove("items[abc]"),
        Err(hjson_edit::HjsonEditError::InvalidPath { .. })
    ));
    
    assert!(matches!(
        doc.remove("items[1x]"),
        Err(hjson_edit::HjsonEditError::InvalidPath { .. })
    ));
}

#[test]
fn string_styles() {
    let doc = HjsonDocument::parse("key: value\n").unwrap();
    
    let preserve = doc.set_with_style("key", "new", StringStyle::Preserve).unwrap();
    let unquoted = doc.set_with_style("key", "new", StringStyle::Unquoted).unwrap();
    let quoted = doc.set_with_style("key", "new", StringStyle::DoubleQuoted).unwrap();
    
    HjsonDocument::parse(&preserve.to_string()).unwrap();
    HjsonDocument::parse(&unquoted.to_string()).unwrap();
    HjsonDocument::parse(&quoted.to_string()).unwrap();
}
