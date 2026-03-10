//! Integration tests for hjson-edit
//!
//! These tests verify the complete workflow from parsing to editing.

use hjson_edit::{
    create_token, green_to_string, parse, parse_to_red, red_to_string, RedNodeEdit, RedNodeExt,
    SyntaxKind,
};

/// HJSON設定ファイルのパース→GreenNode変換→文字列出力の一連のワークフローを検証
#[test]
fn test_full_workflow() {
    // HJSONをパース
    let input = r#"{
            // Configuration
            name: "app",
            port: 8080,
            debug: true
        }"#;

    let red = parse_to_red(input).unwrap();

    // GreenNodeに変換
    let green = red.to_green();

    // 文字列出力
    let output = green_to_string(&green);
    assert_eq!(output, input);
}

/// 値のトークン置換による編集が正しく機能することを検証
///
/// Object > Pair > Value内の文字列 "value" を "new_value" に置換する
#[test]
fn test_edit_and_detect_changes() {
    let input = r#"{"key": "value"}"#;

    let red = parse_to_red(input).unwrap();
    let _green_before = red.to_green();

    // 構造: Root > Value > Object > Pair > Value
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let object = root_value
        .child_by_kind(SyntaxKind::Object)
        .expect("Object not found");
    let pair = object.children_nodes().next().expect("Pair not found");
    let pair_value = pair
        .child_by_kind(SyntaxKind::Value)
        .expect("Pair Value not found");

    // 新しいトークンを作成
    let new_token = create_token(SyntaxKind::String, "\"new_value\"");

    // 子インデックスを見つける
    let value_green = pair_value.to_green();
    let mut target_idx = None;
    for (i, child) in value_green.children().enumerate() {
        if let rowan::NodeOrToken::Token(t) = child {
            if t.text() == "\"value\"" {
                target_idx = Some(i);
                break;
            }
        }
    }

    let target_idx = target_idx.expect("Target token not found");
    let new_value_green = value_green.replace_child(target_idx, new_token.into());

    // 編集が成功したことを確認
    let new_text = green_to_string(&new_value_green);
    assert!(new_text.contains("new_value"));
}

#[test]
fn test_non_rfc_numeric_extensions_are_rejected() {
    for input in [
        "{ hex: 0xFF }",
        "{ val: NaN }",
        "{ val: Infinity }",
        "{ val: -Infinity }",
    ] {
        assert!(parse(input).is_err(), "should reject {input}");
    }
}

/// 3種類のコメント（//, #, /* */）がラウンドトリップで保持されることを検証
#[test]
fn test_comments_preserved() {
    let input = r#"{
            // Line comment
            # Hash comment
            /* Block
               comment */
            key: value
        }"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// HJSONブロック文字列（'''...'''）のパースとラウンドトリップを検証
#[test]
fn test_block_string() {
    let input = "'''multi\nline\ntext'''";

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// 構文エラーのある入力がErrとして正しく報告されることを検証
#[test]
fn test_error_handling() {
    let input = "{invalid";

    let result = parse(input);
    assert!(result.is_err());
}

/// 中括弧付きルートオブジェクトのパース・ラウンドトリップ・AST構造を検証
///
/// Root > Value > Object の階層と、2つ以上のPairが存在することを確認する
#[test]
fn test_root_object_with_braces() {
    let input = r#"{
  one: 100,
  two: 200
}"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);

    // Root > Value > Object の構造を確認
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let object = root_value
        .child_by_kind(SyntaxKind::Object)
        .expect("Object not found");
    assert!(object.children_nodes().count() >= 2); // at least 2 pairs
}

/// ルートレベルの文字列値のパースとラウンドトリップを検証
#[test]
fn test_root_string() {
    let input = "\"Hello, World!\"";

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// ルートレベルの数値のパースとラウンドトリップを検証
#[test]
fn test_root_number() {
    let input = "42";

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// ルートレベルの真偽値のパースとラウンドトリップを検証
#[test]
fn test_root_boolean() {
    let input = "true";

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// ルートレベルのnull値のパースとラウンドトリップを検証
#[test]
fn test_root_null() {
    let input = "null";

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// ルートレベルの配列のパース・ラウンドトリップ・AST構造を検証
///
/// Root > Value > Array の階層と、3つ以上の要素が存在することを確認する
#[test]
fn test_root_array() {
    let input = r#"["first", "second", "third"]"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);

    // Root > Value > Array の構造を確認
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let array = root_value
        .child_by_kind(SyntaxKind::Array)
        .expect("Array not found");
    assert!(array.children_nodes().count() >= 3); // at least 3 elements
}

/// HJSON仕様のブレースレスルートオブジェクト（{}なし）のパースを検証
///
/// ブレースレスの場合、AST構造は Root > Object > Pair[]（Valueノードなし）
#[test]
fn test_root_object_without_braces() {
    let input = r#"one: 100
two: 200"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);

    // AST構造: Root > Object > Pair[]（Valueノードを経由しない）
    let object = red
        .child_by_kind(SyntaxKind::Object)
        .expect("Object not found");
    assert!(
        red.child_by_kind(SyntaxKind::Value).is_none(),
        "Braceless root should not have a Value wrapper"
    );
    assert!(
        object.children_nodes().count() >= 2,
        "Should have at least 2 pairs"
    );
}

/// 深くネストされた構造（4階層のオブジェクト＋配列）のパースとラウンドトリップを検証
#[test]
fn test_complex_nested_structure() {
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
    { name: "item2", value: 200 },
    { name: "item3", value: 300 }
  ]
}"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// HJSONの末尾カンマ（trailing comma）サポートを検証
#[test]
fn test_trailing_comma() {
    let input = r#"{
  a: 1,
  b: 2,
}"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// 同一オブジェクト内の2つのPairに対して連続でトークン置換を行えることを検証
///
/// nameの値 "original" → "modified" と、countの値 0 → 42 を順に編集する
#[test]
fn test_multiple_edits_sequence() {
    let input = r#"{
  name: "original",
  count: 0
}"#;

    let red = parse_to_red(input).unwrap();

    // Root > Value > Object の構造を確認
    let root_value = red
        .child_by_kind(SyntaxKind::Value)
        .expect("Root Value not found");
    let object = root_value
        .child_by_kind(SyntaxKind::Object)
        .expect("Object not found");

    // 2つのPairが存在することを確認
    let pairs: Vec<_> = object.children_nodes().collect();
    assert_eq!(pairs.len(), 2, "Should have 2 pairs");

    // 1つ目のPairのキーが"name"であることを確認
    let first_pair = &pairs[0];
    let first_key = first_pair
        .child_by_kind(SyntaxKind::Key)
        .expect("Key not found");
    assert!(
        first_key.to_string().contains("name"),
        "First key should be 'name', got '{}'",
        first_key.to_string()
    );

    // 1つ目の編集: nameの値を変更
    let first_value = first_pair
        .child_by_kind(SyntaxKind::Value)
        .expect("Value not found");
    let value_green = first_value.to_green();
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
    let new_value_green = value_green.replace_child(target_idx, new_token.into());
    let new_text = green_to_string(&new_value_green);
    assert!(
        new_text.contains("modified"),
        "Edited value should contain 'modified'"
    );
    assert!(
        !new_text.contains("original"),
        "Edited value should not contain 'original'"
    );

    // 2つ目のPairの値も確認可能であること
    let second_pair = &pairs[1];
    let second_value = second_pair
        .child_by_kind(SyntaxKind::Value)
        .expect("Value not found");
    let second_green = second_value.to_green();
    let new_count_token = create_token(SyntaxKind::Integer, "42");

    let mut count_idx = None;
    for (i, child) in second_green.children().enumerate() {
        if let rowan::NodeOrToken::Token(t) = child {
            if t.text() == "0" {
                count_idx = Some(i);
                break;
            }
        }
    }

    let count_idx = count_idx.expect("Count token not found");
    let new_count_green = second_green.replace_child(count_idx, new_count_token.into());
    let count_text = green_to_string(&new_count_green);
    assert!(
        count_text.contains("42"),
        "Edited count should contain '42'"
    );
}

/// 引用符なし文字列（unquoted string）のパースとラウンドトリップを検証
///
/// キーと値の両方に引用符なし文字列が使われるケースをテストする
#[test]
fn test_unquoted_strings() {
    let input = r#"{
  key1: unquoted value,
  key2: another unquoted value,
  "quoted key": "quoted_value"
}"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// HJSONオブジェクトのカンマなし改行区切りを検証
///
/// HJSON仕様では改行がカンマの代わりに区切りとして使える。
/// パーサー修正箇所: parser.rs parse_object() のカンマなし時のエラー処理（L301付近）
#[test]
fn test_object_no_comma_newline_separated() {
    let input = r#"{
  a: 1
  b: 2
  c: 3
}"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// HJSON配列のカンマなし改行区切りを検証
///
/// HJSON仕様では改行がカンマの代わりに区切りとして使える。
/// パーサー修正箇所: parser.rs parse_array() のカンマなし時のエラー処理（L373付近）
#[test]
fn test_array_no_comma_newline_separated() {
    let input = r#"[
  1
  2
  3
]"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// カンマあり/なし混在のオブジェクトを検証
///
/// HJSON仕様では一部のペアにだけカンマがあっても有効。
/// パーサー修正箇所: parser.rs parse_object() のカンマなし時のエラー処理（L301付近）
#[test]
fn test_object_mixed_comma_and_newline() {
    let input = r#"{
  a: 1,
  b: 2
  c: 3,
  d: 4
}"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// 複数行配列の末尾カンマを検証
///
/// 既存の test_trailing_comma はオブジェクト版。これは配列版。
/// パーサー修正箇所: parser.rs parse_array() の末尾カンマ後のeat_trivia（L369付近）
#[test]
fn test_array_trailing_comma_multiline() {
    let input = r#"[
  1,
  2,
  3,
]"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}
