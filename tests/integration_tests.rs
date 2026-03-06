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

/// HJSON特殊値（16進数、NaN、Infinity、-Infinity）のパースとラウンドトリップを検証
#[test]
fn test_special_values() {
    let input = r#"{
            hex: 0xFF,
            nan_val: NaN,
            inf_val: Infinity,
            neg_inf: -Infinity
        }"#;

    let result = parse(input);
    assert!(result.is_ok());

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
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
    // NOTE: 現在パーサーが末尾カンマを正しく処理しないため、末尾カンマなしの形式を使用
    // TODO: パーサー修正後に末尾カンマありのテストケースも追加
    let input = r#"{
  one: 0xFF00AA,
  two: 0x00FF00
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
    // NOTE: 現在パーサーが改行付き配列を正しく処理しないため、1行形式を使用
    // TODO: パーサー修正後に改行付きのテストケースも追加
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
/// パーサー未実装のため現在は失敗する（既知の問題）
#[test]
fn test_root_object_without_braces() {
    // HJSONでは中括弧なしのオブジェクトも許容される
    // TODO: パーサーのparse()メソッドで、先頭がLBraceでない場合にもObjectとしてパースする処理を追加
    // 実装ポイント:
    // - parse()で、先頭トークンがLBraceでない場合、直接Pairをパース
    // - AST構造は Root > Pair[] (Objectノードなし)
    let input = r#"one: 0xFF00AA
two: 0x00FF00"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);

    // AST構造: Root > Pair[] (Objectノードなし)
    // TODO: パーサー修正後に具体的な検証を追加
}

/// 深くネストされた構造（4階層のオブジェクト＋配列）のパースとラウンドトリップを検証
///
/// パーサーの改行付き配列バグにより現在は失敗する（既知の問題）
#[test]
fn test_complex_nested_structure() {
    // HJSONの複雑な入れ子構造のテスト
    // TODO: 以下の修正が必要:
    // 1. オブジェクト内で2つ以上のキーがある場合、カンマが必要だが正しく処理しない問題
    //    - parse_object()で、カンマ後に改行がある場合の処理を修正
    // 2. 配列内の改行を正しく処理しない問題
    //    - parse_array()で、要素間の改行を正しく処理
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

/// 複数の16進数カラー値を持つオブジェクトのパースとラウンドトリップを検証
#[test]
fn test_hex_colors_in_object() {
    // NOTE: 現在パーサーが末尾カンマを正しく処理しないため、末尾カンマなしの形式を使用
    // TODO: パーサー修正後に末尾カンマありのテストケースも追加
    let input = r#"{
  primary: 0xFF5733,
  secondary: 0x33FF57,
  background: 0x3357FF
}"#;

    let red = parse_to_red(input).unwrap();
    let output = red_to_string(&red);
    assert_eq!(output, input);
}

/// HJSONの末尾カンマ（trailing comma）サポートを検証
///
/// パーサーのカンマ＋改行＋}パターンのバグにより現在は失敗する（既知の問題）
#[test]
fn test_trailing_comma() {
    // HJSONの末尾カンマサポートのテスト
    // TODO: パーサーのparse_object()とparse_array()で末尾カンマを正しく処理するように修正
    // 問題: カンマの後にRBraceやRBracketが来る場合、現在はエラーになっている
    // 修正ポイント:
    // - parse_object(): カンマの後にRBraceが来たら正常終了とする
    // - parse_array(): カンマの後にRBracketが来たら正常終了とする
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
    // NOTE: 現在パーサーが末尾カンマを正しく処理しないため、末尾カンマなしの形式を使用
    // TODO: パーサー修正後に末尾カンマありのテストケースも追加
    let input = r#"{
  key1: unquoted_value,
  key2: another_unquoted_value,
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
