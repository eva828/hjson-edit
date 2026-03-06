//! Complex editing scenario tests for hjson-edit
//!
//! These tests verify advanced editing operations.

use hjson_edit::{
    create_token, green_to_string, parse_to_red, red_to_string, GreenNodeEdit, RedNodeEdit,
    RedNodeExt, SyntaxKind,
};

/// 深くネストされたオブジェクトの値を編集できることを検証
///
/// level1 > level2 > target の3階層を辿り、"original" を "modified" に置換する
#[test]
fn test_edit_nested_value() {
    let input = r#"{
  level1: {
    level2: {
      target: "original"
    }
  }
}"#;

    let red = parse_to_red(input).unwrap();

    // Navigate to the nested value
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let object = root_value
        .child_by_kind(SyntaxKind::Object)
        .expect("Object not found");
    let pair1 = object
        .children_nodes()
        .next()
        .expect("First pair not found");
    let value1 = pair1
        .child_by_kind(SyntaxKind::Value)
        .expect("Value1 not found");
    let nested_object = value1
        .child_by_kind(SyntaxKind::Object)
        .expect("Nested Object not found");
    let pair2 = nested_object
        .children_nodes()
        .next()
        .expect("Second pair not found");
    let value2 = pair2
        .child_by_kind(SyntaxKind::Value)
        .expect("Value2 not found");
    let deep_object = value2
        .child_by_kind(SyntaxKind::Object)
        .expect("Deep Object not found");
    let target_pair = deep_object
        .children_nodes()
        .next()
        .expect("Target pair not found");
    let target_value = target_pair
        .child_by_kind(SyntaxKind::Value)
        .expect("Target Value not found");

    // Edit the value
    let target_green = target_value.to_green();
    let new_token = create_token(SyntaxKind::String, "\"modified\"");

    // Find and replace the string token
    let mut target_idx = None;
    for (i, child) in target_green.children().enumerate() {
        if let rowan::NodeOrToken::Token(t) = child {
            if t.text() == "\"original\"" {
                target_idx = Some(i);
                break;
            }
        }
    }

    let target_idx = target_idx.expect("Target token not found");
    let new_value_green = target_green
        .replace_child(target_idx, new_token.into())
        .unwrap();

    // Verify the change
    let new_text = green_to_string(&new_value_green);
    assert!(new_text.contains("modified"));
    assert!(!new_text.contains("original"));
}

/// 複数行配列の要素を個別に編集できることを検証
///
/// 3要素の配列から2番目の要素 "second" を "modified" に置換する。
#[test]
fn test_edit_array_element() {
    let input = r#"[
  "first",
  "second",
  "third"
]"#;

    let red = parse_to_red(input).unwrap();

    // Navigate to the array
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let array = root_value
        .child_by_kind(SyntaxKind::Array)
        .expect("Array not found");

    // Get all array elements (Value nodes)
    let elements: Vec<_> = array.children_nodes().collect();
    assert_eq!(elements.len(), 3, "Should have 3 elements");

    // Edit the second element
    let second_element = &elements[1];
    let element_green = second_element.to_green();
    let new_token = create_token(SyntaxKind::String, "\"modified\"");

    // Find and replace the string token
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
    let new_element_green = element_green
        .replace_child(target_idx, new_token.into())
        .unwrap();

    // Verify the change
    let new_text = green_to_string(&new_element_green);
    assert!(new_text.contains("modified"));
}

/// オブジェクトに新しいキーを追加する操作を低レベルAPIで実装
///
/// 既存Pairの後に新しいPair（Key + Colon + Value）を追加する
#[test]
fn test_edit_add_new_key() {
    let input = r#"{
  existing: "value"
}"#;

    let red = parse_to_red(input).unwrap();

    // Root > Value > Object の構造を確認
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let object = root_value
        .child_by_kind(SyntaxKind::Object)
        .expect("Object not found");

    // 既存のPairが1つ存在することを確認
    let pairs: Vec<_> = object.children_nodes().collect();
    assert_eq!(pairs.len(), 1, "Should have exactly 1 pair initially");

    // ラウンドトリップが保持されることを確認
    let green = red.to_green();
    let output = green_to_string(&green);
    assert_eq!(output, input, "Round-trip should preserve content");

    // 新しいキーを追加するためのPairノードを構築
    // Pair構造: Key(Ident) + Whitespace + Colon + Whitespace + Value(String)
    let new_key_ident = create_token(SyntaxKind::Ident, "new_key");
    let whitespace1 = create_token(SyntaxKind::Whitespace, " ");
    let colon = create_token(SyntaxKind::Colon, ":");
    let whitespace2 = create_token(SyntaxKind::Whitespace, " ");
    let new_value = create_token(SyntaxKind::String, "\"new_value\"");

    let new_pair = hjson_edit::create_node(
        SyntaxKind::Pair,
        vec![
            new_key_ident.into(),
            whitespace1.into(),
            colon.into(),
            whitespace2.into(),
            new_value.into(),
        ],
    );

    // Objectに新しいPairを追加
    // Objectの構造を確認: LBrace, Whitespace(newline + indent), Pair, RBrace
    // 新しいPairを既存Pairの後、RBraceの前に挿入
    let object_green = object.to_green();

    // Objectの子要素を確認
    let children: Vec<_> = object_green.children().collect();
    assert_eq!(
        children.len(),
        4,
        "Object should have 4 children: LBrace, Whitespace, Pair, RBrace"
    );

    // 段階的に挿入（apply_editsの降順ソートによりインデックスが複雑になるため）
    let comma = create_token(SyntaxKind::Comma, ",");
    let newline = create_token(SyntaxKind::Whitespace, "\n  ");

    // 1. RBraceの前にカンマを挿入（インデックス3）
    let object_green = object_green.insert_child(3, comma.into()).unwrap();
    // 2. カンマの後に改行を挿入（インデックス4）
    let object_green = object_green.insert_child(4, newline.into()).unwrap();
    // 3. 改行の後に新しいPairを挿入（インデックス5）
    let new_object_green = object_green.insert_child(5, new_pair.into()).unwrap();
    let new_object_text = green_to_string(&new_object_green);

    // 新しいキーが追加されたことを確認
    assert!(
        new_object_text.contains("new_key"),
        "New key should be present, got: {}",
        new_object_text
    );
    assert!(
        new_object_text.contains("new_value"),
        "New value should be present, got: {}",
        new_object_text
    );
    assert!(
        new_object_text.contains("existing"),
        "Existing key should be preserved, got: {}",
        new_object_text
    );

    // カンマと改行が追加されていることを確認
    assert!(
        new_object_text.contains(","),
        "Comma should be present between pairs, got: {}",
        new_object_text
    );
}

/// オブジェクトのキー名をトークン置換で変更できることを検証
///
/// "old_key" を "new_key" に置換し、結果に新旧のキー名が正しく反映されることを確認する
#[test]
fn test_edit_change_key_name() {
    let input = r#"{
  old_key: "value"
}"#;

    let red = parse_to_red(input).unwrap();

    // Navigate to the key
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let object = root_value
        .child_by_kind(SyntaxKind::Object)
        .expect("Object not found");
    let pair = object.children_nodes().next().expect("Pair not found");
    let key = pair.child_by_kind(SyntaxKind::Key).expect("Key not found");

    // Edit the key
    let key_green = key.to_green();
    let new_token = create_token(SyntaxKind::Ident, "new_key");

    // Find and replace the key token
    let mut target_idx = None;
    for (i, child) in key_green.children().enumerate() {
        if let rowan::NodeOrToken::Token(t) = child {
            if t.text() == "old_key" {
                target_idx = Some(i);
                break;
            }
        }
    }

    let target_idx = target_idx.expect("Key token not found");
    let new_key_green = key_green
        .replace_child(target_idx, new_token.into())
        .unwrap();

    // Verify the change
    let new_text = green_to_string(&new_key_green);
    assert!(new_text.contains("new_key"));
    assert!(!new_text.contains("old_key"));
}

/// 同一オブジェクト内の複数の値を順に編集できることを検証
///
/// 3つのPairそれぞれの値 "valueN" を "modifiedN" に置換し、
/// immutableなRedNodeの元データが変更されないことも確認する
#[test]
fn test_edit_multiple_values() {
    let input = r#"{
  key1: "value1",
  key2: "value2",
  key3: "value3"
}"#;

    let red = parse_to_red(input).unwrap();

    // Get the root object
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let object = root_value
        .child_by_kind(SyntaxKind::Object)
        .expect("Object not found");

    // 各Pairの値を編集し、結果を検証する
    let pairs: Vec<_> = object.children_nodes().collect();
    assert_eq!(pairs.len(), 3, "Should have 3 pairs");

    for (pair_idx, pair) in pairs.iter().enumerate() {
        if let Some(value) = pair.child_by_kind(SyntaxKind::Value) {
            let value_green = value.to_green();

            // 文字列トークンを探して置換
            for (i, child) in value_green.children().enumerate() {
                if let rowan::NodeOrToken::Token(t) = child {
                    let text = t.text();
                    if text.contains("value") {
                        let new_text = text.replace("value", "modified");
                        let new_token = create_token(SyntaxKind::String, &new_text);
                        let new_value = value_green.replace_child(i, new_token.into()).unwrap();

                        // 各編集結果が正しいことを検証
                        let result_text = green_to_string(&new_value);
                        assert!(
                            result_text.contains("modified"),
                            "Pair {} should contain 'modified', got '{}'",
                            pair_idx,
                            result_text
                        );
                        assert!(
                            !result_text.contains("value"),
                            "Pair {} should not contain 'value', got '{}'",
                            pair_idx,
                            result_text
                        );
                        break;
                    }
                }
            }
        }
    }

    // 元のRedNodeはimmutableなので、元の入力が保持されていることも確認
    let output = red_to_string(&red);
    assert_eq!(output, input, "Original should be unchanged");
}

/// パース→Green Tree→文字列のラウンドトリップで空白が保持されることを検証
///
/// 不規則な空白（余分なスペース・空行）とコメントが含まれる入力が
/// そのまま復元されることを確認する
#[test]
fn test_edit_preserves_whitespace() {
    let input = r#"{
  
  // This is a comment
  key:    "value"    
  
}"#;

    let red = parse_to_red(input).unwrap();
    let green = red.to_green();

    // Simple round-trip should preserve everything including whitespace
    let output = green_to_string(&green);
    assert_eq!(output, input, "Whitespace and comments should be preserved");
}

/// ラウンドトリップで3種類のコメント（//, #, /* */）が保持されることを検証
#[test]
fn test_edit_preserves_comments() {
    let input = r#"{
  // Line comment
  # Hash comment
  /* Block comment */
  key: "value"
}"#;

    let red = parse_to_red(input).unwrap();
    let green = red.to_green();

    // Round-trip should preserve all comments
    let output = green_to_string(&green);
    assert!(
        output.contains("// Line comment"),
        "Line comment should be preserved"
    );
    assert!(
        output.contains("# Hash comment"),
        "Hash comment should be preserved"
    );
    assert!(
        output.contains("/* Block comment */"),
        "Block comment should be preserved"
    );
}

/// オブジェクトの配列内の値を編集できることを検証
///
/// 配列の最初のオブジェクトの値 100 を 999 に置換し、
/// 元のRedNodeが変更されないことも確認する。
#[test]
fn test_edit_in_array_of_objects() {
    let input = r#"[
  { name: "item1", value: 100 },
  { name: "item2", value: 200 }
]"#;

    let red = parse_to_red(input).unwrap();

    // Navigate to array
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let array = root_value
        .child_by_kind(SyntaxKind::Array)
        .expect("Array not found");

    // Get array elements
    let elements: Vec<_> = array.children_nodes().collect();
    assert_eq!(elements.len(), 2);

    // Edit first element's value (value: 100)
    let first_obj = elements[0]
        .child_by_kind(SyntaxKind::Object)
        .expect("Object not found");
    let mut pairs: Vec<_> = first_obj.children_nodes().collect();
    assert_eq!(pairs.len(), 2, "Should have 2 pairs: name and value");
    let value_pair = pairs.pop().expect("Value pair not found"); // 2番目のPair (value: 100)
    let value_node = value_pair
        .child_by_kind(SyntaxKind::Value)
        .expect("Value not found");

    let value_green = value_node.to_green();
    let new_token = create_token(SyntaxKind::Integer, "999");

    // Find and replace the number token
    let mut target_idx = None;
    for (i, child) in value_green.children().enumerate() {
        if let rowan::NodeOrToken::Token(t) = child {
            if t.text() == "100" {
                target_idx = Some(i);
                break;
            }
        }
    }

    let target_idx = target_idx.expect("Target token (100) not found");
    let new_value_green = value_green
        .replace_child(target_idx, new_token.into())
        .unwrap();

    // 編集結果が正しいことを検証
    let new_text = green_to_string(&new_value_green);
    assert!(
        new_text.contains("999"),
        "Edited value should contain '999', got '{}'",
        new_text
    );
    assert!(
        !new_text.contains("100"),
        "Edited value should not contain '100', got '{}'",
        new_text
    );

    // 元のRedNodeはimmutableなので入力が保持されていることも確認
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// 16進数値をトークン置換で変更できることを検証
///
/// HexInteger トークン 0xFF5733 を 0x00FF00 に置換する
#[test]
fn test_edit_hex_value() {
    let input = r#"{
  color: 0xFF5733
}"#;

    let red = parse_to_red(input).unwrap();

    // Navigate to the value
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let object = root_value
        .child_by_kind(SyntaxKind::Object)
        .expect("Object not found");
    let pair = object.children_nodes().next().expect("Pair not found");
    let value = pair
        .child_by_kind(SyntaxKind::Value)
        .expect("Value not found");

    let value_green = value.to_green();
    let new_token = create_token(SyntaxKind::HexInteger, "0x00FF00");

    // Find and replace the hex token
    let mut target_idx = None;
    for (i, child) in value_green.children().enumerate() {
        if let rowan::NodeOrToken::Token(t) = child {
            if t.text() == "0xFF5733" {
                target_idx = Some(i);
                break;
            }
        }
    }

    let target_idx = target_idx.expect("Hex token not found");
    let new_value_green = value_green
        .replace_child(target_idx, new_token.into())
        .unwrap();

    // Verify the change
    let new_text = green_to_string(&new_value_green);
    assert!(new_text.contains("0x00FF00"));
}
