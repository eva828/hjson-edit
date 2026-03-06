//! File I/O integration tests for hjson-edit
//!
//! These tests verify reading and writing HJSON files.

use hjson_edit::{parse, parse_to_red, red_to_string};
use std::fs;
use std::path::Path;

const FIXTURES_DIR: &str = "tests/fixtures";

/// 中括弧付きルートオブジェクトのfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_root_with_braces() {
    let path = format!("{}/root_with_braces.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// ブレースレスルートオブジェクトのfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_root_without_braces() {
    let path = format!("{}/root_without_braces.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// ルートが文字列値のfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_root_string() {
    let path = format!("{}/root_string.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// ルートが数値のfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_root_number() {
    let path = format!("{}/root_number.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// ルートが真偽値のfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_root_boolean() {
    let path = format!("{}/root_boolean.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// ルートがnullのfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_root_null() {
    let path = format!("{}/root_null.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// ルートが配列のfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_root_array() {
    let path = format!("{}/root_array.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// 深いネスト構造のfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_complex_nested() {
    let path = format!("{}/complex_nested.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// コメント付きHJSONのfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_with_comments() {
    let path = format!("{}/with_comments.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// 構文エラーのあるfixtureファイルがErrとして報告されることを検証
#[test]
fn test_read_invalid_syntax() {
    let path = format!("{}/invalid_syntax.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    // This file should fail to parse
    let result = parse(&content);
    assert!(result.is_err(), "Invalid syntax should fail to parse");
}

/// 16進数カラー値のfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_hex_colors() {
    let path = format!("{}/hex_colors.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// 引用符なし文字列のfixtureファイルを読み込みラウンドトリップを検証
#[test]
fn test_read_unquoted_strings() {
    let path = format!("{}/unquoted_strings.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// カンマなし改行区切りオブジェクトのfixtureファイルを読み込みラウンドトリップを検証
///
/// HJSON仕様では改行がカンマの代わりに区切りとして使える。
/// パーサー修正箇所: parser.rs parse_object() のカンマなし時のエラー処理（L301付近）
#[test]
fn test_read_no_comma_object() {
    let path = format!("{}/no_comma_object.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// カンマなし改行区切り配列のfixtureファイルを読み込みラウンドトリップを検証
///
/// HJSON仕様では改行がカンマの代わりに区切りとして使える。
/// パーサー修正箇所: parser.rs parse_array() のカンマなし時のエラー処理（L373付近）
#[test]
fn test_read_no_comma_array() {
    let path = format!("{}/no_comma_array.hjson", FIXTURES_DIR);
    let content = fs::read_to_string(&path).expect("Failed to read file");

    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");
}

/// ファイルへの書き込みと読み込みのラウンドトリップが一致することを検証
///
/// /tmp に一時ファイルを作成し、書き込み→読み込み→パース→文字列化の一連を確認する
#[test]
fn test_write_and_read_roundtrip() {
    use std::io::Write;

    let test_content = r#"{
  test: "value",
  number: 42
}"#;

    let temp_path = "/tmp/test_roundtrip.hjson";

    // Write to file
    {
        let mut file = fs::File::create(temp_path).expect("Failed to create file");
        file.write_all(test_content.as_bytes())
            .expect("Failed to write");
    }

    // Read back
    let content = fs::read_to_string(temp_path).expect("Failed to read file");
    let red = parse_to_red(&content).expect("Failed to parse");
    let output = red_to_string(&red);

    assert_eq!(output, content, "Round-trip should preserve content");

    // Cleanup
    fs::remove_file(temp_path).ok();
}

/// 全14種のfixtureファイルが tests/fixtures/ に存在することを検証
#[test]
fn test_all_fixtures_exist() {
    let fixtures = vec![
        "root_with_braces.hjson",
        "root_without_braces.hjson",
        "root_string.hjson",
        "root_number.hjson",
        "root_boolean.hjson",
        "root_null.hjson",
        "root_array.hjson",
        "complex_nested.hjson",
        "with_comments.hjson",
        "invalid_syntax.hjson",
        "hex_colors.hjson",
        "unquoted_strings.hjson",
        "no_comma_object.hjson",
        "no_comma_array.hjson",
    ];

    for fixture in fixtures {
        let path = format!("{}/{}", FIXTURES_DIR, fixture);
        assert!(
            Path::new(&path).exists(),
            "Fixture file should exist: {}",
            fixture
        );
    }
}
