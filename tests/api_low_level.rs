//! Low-level API tests - Green/Red tree operations

use hjson_edit::{
    create_token, parse_to_red, GreenNodeEdit, GreenNodeExt, RedNodeCoreExt, SyntaxKind,
};

#[test]
fn replace_token_in_value() {
    let input = r#"{"key": "original"}"#;
    let red = parse_to_red(input).unwrap();
    
    let root_value = red.child_by_kind(SyntaxKind::Value).unwrap();
    let object = root_value.child_by_kind(SyntaxKind::Object).unwrap();
    let pair = object.children_nodes().next().unwrap();
    let value = pair.child_by_kind(SyntaxKind::Value).unwrap();
    
    let value_green = value.to_green();
    let new_token = create_token(SyntaxKind::String, "\"modified\"");
    
    let mut target_idx = None;
    for (i, child) in value_green.children().enumerate() {
        if let rowan::NodeOrToken::Token(t) = child {
            if t.text() == "\"original\"" {
                target_idx = Some(i);
                break;
            }
        }
    }
    
    let target_idx = target_idx.expect("Target token not found");
    let new_value = value_green.replace_child(target_idx, new_token.into()).unwrap();
    
    assert!(new_value.text().contains("modified"));
    assert!(!new_value.text().contains("original"));
}

#[test]
fn navigate_nested_structure() {
    let input = r#"{"level1": {"level2": {"target": "value"}}}"#;
    let red = parse_to_red(input).unwrap();
    
    let root_value = red.child_by_kind(SyntaxKind::Value).unwrap();
    let object1 = root_value.child_by_kind(SyntaxKind::Object).unwrap();
    let pair1 = object1.children_nodes().next().unwrap();
    let value1 = pair1.child_by_kind(SyntaxKind::Value).unwrap();
    let object2 = value1.child_by_kind(SyntaxKind::Object).unwrap();
    let pair2 = object2.children_nodes().next().unwrap();
    let value2 = pair2.child_by_kind(SyntaxKind::Value).unwrap();
    let object3 = value2.child_by_kind(SyntaxKind::Object).unwrap();
    let pair3 = object3.children_nodes().next().unwrap();
    
    assert!(pair3.to_string().contains("target"));
}

#[test]
fn preserves_whitespace_and_comments() {
    let input = r#"{
  // This is a comment
  key:    "value"    
  
}"#;
    
    let red = parse_to_red(input).unwrap();
    let green = red.to_green();
    let output = green.text();
    
    assert_eq!(output, input, "Whitespace and comments should be preserved");
}

#[test]
fn preserves_all_comment_types() {
    let input = r#"{
  // Line comment
  # Hash comment
  /* Block comment */
  key: "value"
}"#;
    
    let red = parse_to_red(input).unwrap();
    let output = red.to_string();
    
    assert!(output.contains("// Line comment"));
    assert!(output.contains("# Hash comment"));
    assert!(output.contains("/* Block comment */"));
}

#[test]
fn edit_array_element() {
    let input = r#"["first", "second", "third"]"#;
    let red = parse_to_red(input).unwrap();
    
    let root_value = red.child_by_kind(SyntaxKind::Value).unwrap();
    let array = root_value.child_by_kind(SyntaxKind::Array).unwrap();
    
    let elements: Vec<_> = array.children_nodes().collect();
    assert_eq!(elements.len(), 3);
    
    let second = &elements[1];
    let element_green = second.to_green();
    let new_token = create_token(SyntaxKind::String, "\"modified\"");
    
    let mut target_idx = None;
    for (i, child) in element_green.children().enumerate() {
        if let rowan::NodeOrToken::Token(t) = child {
            if t.text() == "\"second\"" {
                target_idx = Some(i);
                break;
            }
        }
    }
    
    let target_idx = target_idx.expect("Target token not found");
    let new_element = element_green.replace_child(target_idx, new_token.into()).unwrap();
    
    assert!(new_element.text().contains("modified"));
}

#[test]
fn original_unchanged_after_edit() {
    let input = r#"{"key": "value"}"#;
    let red = parse_to_red(input).unwrap();
    
    let green = red.to_green();
    let _ = green.text();
    
    assert_eq!(red.to_string(), input);
}

#[test]
fn block_string_roundtrip() {
    let input = "'''multi\nline\ntext'''";
    let red = parse_to_red(input).unwrap();
    let output = red.to_string();
    
    assert_eq!(output, input);
}

#[test]
fn change_key_name() {
    let input = r#"{"old_key": "value"}"#;
    let red = parse_to_red(input).unwrap();
    
    let root_value = red.child_by_kind(SyntaxKind::Value).unwrap();
    let object = root_value.child_by_kind(SyntaxKind::Object).unwrap();
    let pair = object.children_nodes().next().unwrap();
    let key = pair.child_by_kind(SyntaxKind::Key).unwrap();
    
    let key_green = key.to_green();
    let new_token = create_token(SyntaxKind::Ident, "new_key");
    
    let mut target_idx = None;
    for (i, child) in key_green.children().enumerate() {
        if let rowan::NodeOrToken::Token(t) = child {
            if t.text() == "\"old_key\"" || t.text() == "old_key" {
                target_idx = Some(i);
                break;
            }
        }
    }
    
    let target_idx = target_idx.expect("Key token not found");
    let new_key = key_green.replace_child(target_idx, new_token.into()).unwrap();
    
    assert!(new_key.text().contains("new_key"));
    assert!(!new_key.text().contains("old_key"));
}

#[test]
fn line_range_tracking() {
    use hjson_edit::RedNodeCoreExt;
    
    // Single line document
    let input = r#"{ "key": "value" }"#;
    let red = parse_to_red(input).unwrap();
    let (start, end) = red.line_range(input);
    assert_eq!(start, 1);
    assert_eq!(end, 1);
    
    // Multi-line document
    let input = r#"{
  "key1": "value1",
  "key2": "value2"
}"#;
    let red = parse_to_red(input).unwrap();
    let (start, end) = red.line_range(input);
    assert_eq!(start, 1);
    assert_eq!(end, 4);
}
