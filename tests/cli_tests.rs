//! CLI integration tests for hjson-edit
//!
//! These tests verify the command-line interface works correctly.

use std::fs;
use std::process::Command;

fn get_binary_path() -> String {
    // Get the path to the compiled binary
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test executable name
    path.pop(); // Remove 'deps'
    path.push("hjson-edit");
    path.to_str().unwrap().to_string()
}

/// parseコマンドで有効なHJSONファイルを渡すと "Parsed successfully" が出力されることを検証
#[test]
fn test_cli_parse_command() {
    let binary = get_binary_path();

    // Create a temporary test file
    let test_content = r#"{
  name: "test",
  value: 42
}"#;
    let temp_path = "/tmp/test_parse.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["parse", temp_path])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Parsed successfully"),
        "Expected success message"
    );

    // Cleanup
    fs::remove_file(temp_path).ok();
}

/// formatコマンドでフォーマット保持出力（入力と同一の文字列）が得られることを検証
#[test]
fn test_cli_format_command() {
    let binary = get_binary_path();

    // Create a temporary test file
    let test_content = r#"{
  name: "test",
  value: 42
}"#;
    let temp_path = "/tmp/test_format.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["format", temp_path])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Format should preserve the input exactly
    assert_eq!(
        stdout.trim(),
        test_content.trim(),
        "Format should preserve input"
    );

    // Cleanup
    fs::remove_file(temp_path).ok();
}

// editコマンドのテストは以下で実装

/// 引数なしで実行するとUsageメッセージが表示され、終了コードが非0であることを検証
#[test]
fn test_cli_no_args() {
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .output()
        .expect("Failed to execute command");

    // Should fail and show usage
    assert!(!output.status.success(), "Should fail without args");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage"), "Should show usage message");
}

/// 未知のコマンドを渡すと "unrecognized subcommand" エラーで終了することを検証
#[test]
fn test_cli_unknown_command() {
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .args(["unknown"])
        .output()
        .expect("Failed to execute command");

    // Should fail with unrecognized subcommand message
    assert!(!output.status.success(), "Should fail with unknown command");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unrecognized subcommand"),
        "Should show unrecognized subcommand message"
    );
}

/// 存在しないファイルを指定すると "Failed to read file" エラーで終了することを検証
#[test]
fn test_cli_parse_missing_file() {
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .args(["parse", "/tmp/nonexistent_file.hjson"])
        .output()
        .expect("Failed to execute command");

    // Should fail because file doesn't exist
    assert!(!output.status.success(), "Should fail with missing file");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to read file"),
        "Should show file read error"
    );
}

/// 構文エラーのあるHJSONファイルを渡すと "Parse errors" で終了することを検証
#[test]
fn test_cli_parse_invalid_hjson() {
    let binary = get_binary_path();

    // Create a file with invalid HJSON
    let test_content = "{invalid json";
    let temp_path = "/tmp/test_invalid.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["parse", temp_path])
        .output()
        .expect("Failed to execute command");

    // Should fail with parse errors
    assert!(!output.status.success(), "Should fail with parse error");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Parse errors"), "Should show parse errors");

    // Cleanup
    fs::remove_file(temp_path).ok();
}

// =============================================================================
// Edit Command Tests
// =============================================================================

/// editコマンドで既存キーの値を変更できることを検証
#[test]
fn test_edit_object_set_existing() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test", "value": 42}"#;
    let temp_path = "/tmp/test_edit_set.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--object-set", "value", "100"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("100"), "Value should be updated to 100");
    assert!(!stdout.contains("42"), "Old value should be gone");

    fs::remove_file(temp_path).ok();
}

/// --object-setで改行・インデントが保持されることを検証（バグ修正テスト）
#[test]
fn test_edit_object_set_preserves_newlines() {
    let binary = get_binary_path();

    // 改行ベースのHJSONフォーマット
    let test_content = "{\n  DisplayName: test\n  Icon: test\n  ItemType: Test\n}";
    let temp_path = "/tmp/test_edit_newlines.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--object-set", "DisplayName", "\"ae\""])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // 値が変更されていることを確認
    assert!(
        stdout.contains("\"ae\""),
        "Value should be updated to \"ae\""
    );
    assert!(
        !stdout.contains("DisplayName: test"),
        "Old value should be gone"
    );

    // 改行・インデントが保持されていることを確認（バグ修正の検証）
    assert!(
        stdout.contains("\n  Icon:"),
        "Newline and indent before Icon should be preserved"
    );
    assert!(
        stdout.contains("\n  ItemType:"),
        "Newline and indent before ItemType should be preserved"
    );

    // 全体の構造が保持されていることを確認（コロンの後のスペースはcreate_pair_nodeの挙動による）
    // 重要なのは改行・インデントが保持されること
    assert!(
        stdout.starts_with("{\n  DisplayName:"),
        "Format should start with opening brace and proper indent. Got:\n{}",
        stdout
    );

    fs::remove_file(temp_path).ok();
}

/// editコマンドで新規キーを追加できることを検証
#[test]
fn test_edit_object_set_new_key() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let temp_path = "/tmp/test_edit_new.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--object-set", "value", "42"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("value"), "New key should exist");
    assert!(stdout.contains("42"), "New value should be 42");

    fs::remove_file(temp_path).ok();
}

/// editコマンドでキーを削除できることを検証
#[test]
fn test_edit_object_remove() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test", "value": 42}"#;
    let temp_path = "/tmp/test_edit_remove.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--object-remove", "value"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("value"), "Removed key should not exist");
    assert!(stdout.contains("name"), "Other key should still exist");

    fs::remove_file(temp_path).ok();
}

/// 複数行オブジェクトからキー削除時に空行が残らないことを検証
#[test]
fn test_edit_object_remove_multiline_preserves_layout() {
    let binary = get_binary_path();

    let test_content = "Items: {\n\tRegrowthHamaxe: {\n\t\tDisplayName: 成長のハンマーアックス\n\t\tTooltip:\n\t\t\t'''\n\t\t\t土に草を生やす\n\t\t\t植物を多めに収集できる\n\t\t\t木を伐採するときにドングリを植える\n\t\t\t'''\n\t}\n}\n";
    let expected = "Items: {\n\tRegrowthHamaxe: {\n\t\tTooltip:\n\t\t\t'''\n\t\t\t土に草を生やす\n\t\t\t植物を多めに収集できる\n\t\t\t木を伐採するときにドングリを植える\n\t\t\t'''\n\t}\n}\n";
    let temp_path = std::env::temp_dir().join("test_edit_remove_multiline.hjson");
    fs::write(&temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path.to_str().unwrap(),
            "--object-remove",
            "Items.RegrowthHamaxe.DisplayName",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);

    fs::remove_file(temp_path).ok();
}

/// カンマ区切り object の先頭要素削除で先頭にカンマが残らないことを検証
#[test]
fn test_edit_object_remove_inline_comma_object() {
    let binary = get_binary_path();

    let test_content = r#"{a: 1, b: 2}"#;
    let temp_path = std::env::temp_dir().join("test_edit_remove_inline_comma.hjson");
    fs::write(&temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path.to_str().unwrap(), "--object-remove", "a"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "{b: 2}");

    fs::remove_file(temp_path).ok();
}

/// 末尾 standalone trivia が削除後に重複しないことを検証
#[test]
fn test_edit_object_remove_does_not_duplicate_trailing_standalone_trivia() {
    let binary = get_binary_path();

    let test_content = "{\n\ta: 1\n\n\t# footer\n}";
    let expected = "{\n\t# footer\n}";
    let temp_path = std::env::temp_dir().join("test_edit_remove_trailing_standalone.hjson");
    fs::write(&temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path.to_str().unwrap(), "--object-remove", "a"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);

    fs::remove_file(temp_path).ok();
}

/// 複数行配列の要素削除で空行が残らないことを検証
#[test]
fn test_edit_array_remove_multiline_preserves_layout() {
    let binary = get_binary_path();

    let test_content = "{\n\titems: [\n\t\t1\n\t\t2\n\t\t3\n\t]\n}";
    let expected = "{\n\titems: [\n\t\t1\n\t\t3\n\t]\n}";
    let temp_path = std::env::temp_dir().join("test_edit_array_remove_multiline.hjson");
    fs::write(&temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path.to_str().unwrap(),
            "--array-remove",
            "items",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);

    fs::remove_file(temp_path).ok();
}

/// 配列削除で他の要素間の意図的な空行を潰さないことを検証
#[test]
fn test_edit_array_remove_preserves_existing_blank_lines() {
    let binary = get_binary_path();

    let test_content = "{\n\titems: [\n\t\t1\n\n\t\t2\n\n\t\t3\n\t]\n}";
    let expected = "{\n\titems: [\n\t\t1\n\n\t\t3\n\t]\n}";
    let temp_path = std::env::temp_dir().join("test_edit_array_remove_blank_lines.hjson");
    fs::write(&temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path.to_str().unwrap(),
            "--array-remove",
            "items",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);

    fs::remove_file(temp_path).ok();
}

/// 配列削除で削除対象に付いた inline comment を残さないことを検証
#[test]
fn test_edit_array_remove_drops_inline_comment_with_entry() {
    let binary = get_binary_path();

    let test_content = "{\n\titems: [\n\t\t1 # keep\n\t\t2 # drop\n\t\t3\n\t]\n}";
    let temp_path = std::env::temp_dir().join("test_edit_array_remove_comment.hjson");
    fs::write(&temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path.to_str().unwrap(),
            "--array-remove",
            "items",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# keep"));
    assert!(!stdout.contains("# drop"));

    fs::remove_file(temp_path).ok();
}

/// 複数行配列への append で改行レイアウトが維持されることを検証
#[test]
fn test_edit_array_append_multiline_preserves_layout() {
    let binary = get_binary_path();

    let test_content = "{\n\titems: [\n\t\t1\n\t\t2\n\t]\n}";
    let expected = "{\n\titems: [\n\t\t1\n\t\t2\n\t\t3\n\t]\n}";
    let temp_path = std::env::temp_dir().join("test_edit_array_append_multiline.hjson");
    fs::write(&temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path.to_str().unwrap(),
            "--array-append",
            "items",
            "3",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);

    fs::remove_file(temp_path).ok();
}

/// 複数行配列への insert で改行レイアウトが維持されることを検証
#[test]
fn test_edit_array_insert_multiline_preserves_layout() {
    let binary = get_binary_path();

    let test_content = "{\n\titems: [\n\t\t1\n\t\t3\n\t]\n}";
    let expected = "{\n\titems: [\n\t\t1\n\t\t2\n\t\t3\n\t]\n}";
    let temp_path = std::env::temp_dir().join("test_edit_array_insert_multiline.hjson");
    fs::write(&temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path.to_str().unwrap(),
            "--array-insert",
            "items",
            "1",
            "2",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);

    fs::remove_file(temp_path).ok();
}

/// 複数行オブジェクトの既存キー更新でレイアウトが崩れないことを検証
#[test]
fn test_edit_object_set_multiline_preserves_layout() {
    let binary = get_binary_path();

    let test_content =
        "Items: {\n\tRegrowthHamaxe: {\n\t\tDisplayName: old\n\t\tTooltip: keep\n\t}\n}\n";
    let temp_path = std::env::temp_dir().join("test_edit_object_set_multiline.hjson");
    fs::write(&temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path.to_str().unwrap(),
            "--object-set",
            "Items.RegrowthHamaxe.DisplayName",
            "new",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\t\tDisplayName:"));
    assert!(stdout.contains("\t\tTooltip: keep"));
    assert!(stdout.contains("\n\t\tTooltip: keep\n"));

    fs::remove_file(temp_path).ok();
}

/// --setエイリアスが--object-setと同じ動作をすることを検証
#[test]
fn test_edit_alias_set() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let temp_path = "/tmp/test_edit_alias.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "value", "42"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("value"));
    assert!(stdout.contains("42"));

    fs::remove_file(temp_path).ok();
}

/// --removeエイリアスが--object-removeと同じ動作をすることを検証
#[test]
fn test_edit_alias_remove() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test", "value": 42}"#;
    let temp_path = "/tmp/test_edit_alias2.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--remove", "value"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("value"));

    fs::remove_file(temp_path).ok();
}

/// --array-appendで配列に要素を追加できることを検証
#[test]
fn test_edit_array_append() {
    let binary = get_binary_path();

    let test_content = r#"{"items": [1, 2]}"#;
    let temp_path = "/tmp/test_edit_append.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--array-append", "items", "3"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("3"), "Appended value should exist");

    fs::remove_file(temp_path).ok();
}

/// --array-insertで配列に要素を挿入できることを検証
#[test]
fn test_edit_array_insert() {
    let binary = get_binary_path();

    let test_content = r#"{"items": [1, 3]}"#;
    let temp_path = "/tmp/test_edit_insert.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--array-insert", "items", "1", "2"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Check that 2 is between 1 and 3 (order matters in HJSON arrays)
    let pos1 = stdout.find('1').unwrap_or(0);
    let pos2 = stdout.find('2').unwrap_or(0);
    let pos3 = stdout.find('3').unwrap_or(0);
    assert!(pos1 < pos2 && pos2 < pos3, "2 should be between 1 and 3");

    fs::remove_file(temp_path).ok();
}

/// --array-removeで配列の要素を削除できることを検証
#[test]
fn test_edit_array_remove() {
    let binary = get_binary_path();

    let test_content = r#"{"items": [1, 2, 3]}"#;
    let temp_path = "/tmp/test_edit_array_remove.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--array-remove", "items", "1"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1"), "First item should still exist");
    assert!(!stdout.contains("2"), "Removed item should not exist");
    assert!(stdout.contains("3"), "Third item should still exist");

    fs::remove_file(temp_path).ok();
}

/// 複数の操作が正しい順序で実行されることを検証
#[test]
fn test_edit_chain_order() {
    let binary = get_binary_path();

    let test_content = r#"{"a": 1, "b": 2, "c": 3}"#;
    let temp_path = "/tmp/test_edit_chain.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path,
            "--object-set",
            "a",
            "10",
            "--object-remove",
            "b",
            "--object-set",
            "c",
            "30",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("a"));
    assert!(!stdout.contains("b"));
    assert!(stdout.contains("c"));

    fs::remove_file(temp_path).ok();
}

/// 整数の型推論が正しく動作することを検証
#[test]
fn test_edit_type_inference_int() {
    let binary = get_binary_path();

    let test_content = r#"{}"#;
    let temp_path = "/tmp/test_edit_type_int.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "x", "42"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("42"));

    fs::remove_file(temp_path).ok();
}

/// 浮動小数点の型推論が正しく動作することを検証
#[test]
fn test_edit_type_inference_float() {
    let binary = get_binary_path();

    let test_content = r#"{}"#;
    let temp_path = "/tmp/test_edit_type_float.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "x", "3.14"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("3.14"));

    fs::remove_file(temp_path).ok();
}

/// boolの型推論が正しく動作することを検証
#[test]
fn test_edit_type_inference_bool() {
    let binary = get_binary_path();

    let test_content = r#"{}"#;
    let temp_path = "/tmp/test_edit_type_bool.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "x", "true"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("true"));

    fs::remove_file(temp_path).ok();
}

/// nullの型推論が正しく動作することを検証
#[test]
fn test_edit_type_inference_null() {
    let binary = get_binary_path();

    let test_content = r#"{}"#;
    let temp_path = "/tmp/test_edit_type_null.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "x", "null"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("null"));

    fs::remove_file(temp_path).ok();
}

/// 16進数の型推論が正しく動作することを検証
#[test]
fn test_edit_type_inference_hex() {
    let binary = get_binary_path();

    let test_content = r#"{}"#;
    let temp_path = "/tmp/test_edit_type_hex.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "x", "0xFF"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0xFF") || stdout.contains("255"),
        "Hex value should be recognized"
    );

    fs::remove_file(temp_path).ok();
}

/// :str型指定で数値を文字列として設定できることを検証
#[test]
fn test_edit_type_hint_str() {
    let binary = get_binary_path();

    let test_content = r#"{}"#;
    let temp_path = "/tmp/test_edit_hint_str.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "x:str", "42"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"42\""),
        "Value should be quoted as string"
    );

    fs::remove_file(temp_path).ok();
}

/// :null型指定でnull値を設定できることを検証
#[test]
fn test_edit_type_hint_null() {
    let binary = get_binary_path();

    let test_content = r#"{}"#;
    let temp_path = "/tmp/test_edit_hint_null.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "x:null", "null"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("null"));

    fs::remove_file(temp_path).ok();
}

/// --comment-setでキー直前のコメントを設定できることを検証
#[test]
fn test_edit_comment_set_before() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let temp_path = "/tmp/test_edit_comment.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path,
            "--comment-set",
            "name",
            "This is a comment",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("This is a comment"),
        "Comment should be added"
    );

    fs::remove_file(temp_path).ok();
}

/// --comment-set :headerでヘッダーコメントを設定できることを検証
#[test]
fn test_edit_comment_set_header() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let temp_path = "/tmp/test_edit_header.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--comment-set", ":header", "Config file"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Config file"),
        "Header comment should be added"
    );

    fs::remove_file(temp_path).ok();
}

/// --comment-style hashでコメントスタイルを変更できることを検証
#[test]
fn test_edit_comment_style_hash() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let temp_path = "/tmp/test_edit_style.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path,
            "--comment-style",
            "hash",
            "--comment-set",
            "name",
            "Comment",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# "), "Hash-style comment should be used");

    fs::remove_file(temp_path).ok();
}

/// --in-placeでファイルが上書きされることを検証
#[test]
fn test_edit_in_place() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let temp_path = "/tmp/test_edit_inplace.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--in-place", "--set", "value", "42"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    let updated = fs::read_to_string(temp_path).unwrap();
    assert!(updated.contains("value"));
    assert!(updated.contains("42"));

    fs::remove_file(temp_path).ok();
}

/// --outputで別ファイルに出力されることを検証
#[test]
fn test_edit_output_file() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let input_path = "/tmp/test_edit_input.hjson";
    let output_path = "/tmp/test_edit_output.hjson";
    fs::write(input_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            input_path,
            "--output",
            output_path,
            "--set",
            "value",
            "42",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    // Original file should be unchanged
    let original = fs::read_to_string(input_path).unwrap();
    assert!(!original.contains("value"));

    // Output file should have the new key
    let output_content = fs::read_to_string(output_path).unwrap();
    assert!(output_content.contains("value"));
    assert!(output_content.contains("42"));

    fs::remove_file(input_path).ok();
    fs::remove_file(output_path).ok();
}

/// デフォルトでstdoutに出力されることを検証
#[test]
fn test_edit_stdout_default() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let temp_path = "/tmp/test_edit_stdout.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "value", "42"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    // stdout should have the content
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("value"));

    // Original file should be unchanged
    let original = fs::read_to_string(temp_path).unwrap();
    assert!(!original.contains("value"));

    fs::remove_file(temp_path).ok();
}

/// データ操作後もコメントが保持されることを検証
#[test]
fn test_edit_preserves_comments() {
    let binary = get_binary_path();

    let test_content = r#"// Header comment
{
    // Key comment
    name: "test"
}"#;
    let temp_path = "/tmp/test_edit_preserve.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "value", "42"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Comments should be preserved
    assert!(
        stdout.contains("Header comment"),
        "Header comment should be preserved"
    );
    assert!(
        stdout.contains("Key comment"),
        "Key comment should be preserved"
    );

    fs::remove_file(temp_path).ok();
}
