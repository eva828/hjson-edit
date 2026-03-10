//! CLI integration tests for hjson-edit
//!
//! These tests verify the command-line interface works correctly.

use std::fs;
use std::process::Command;

use hjson_edit::{HjsonDocument, HjsonValue};

fn get_binary_path() -> String {
    // Get the path to the compiled binary
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test executable name
    path.pop(); // Remove 'deps'
    path.push("hjson-edit");
    path.to_str().unwrap().to_string()
}

fn assert_stdout_parses(output: &[u8]) -> HjsonDocument {
    let stdout = String::from_utf8_lossy(output);
    HjsonDocument::parse(&stdout).unwrap()
}

/// parseコマンドで有効なHJSONファイルを渡すと "Parsed successfully" が出力されることを検証
#[test]
fn test_cli_parse_command() {
    let binary = get_binary_path();

    // Create a temporary test file
    let test_content = "{\n  name: \"test\",\n\n  value: 42\n}";
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

/// formatコマンドで文書全体が整形されることを検証
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
    assert!(stdout.contains("name: \"test\""));
    assert!(stdout.contains("value: 42"));
    assert!(!stdout.contains("\n\n\n"));

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
    assert!(stdout.contains("Icon: test"));
    assert!(stdout.contains("ItemType: Test"));

    // 全体の構造が保持されていることを確認（コロンの後のスペースはcreate_pair_nodeの挙動による）
    // 重要なのは改行・インデントが保持されること
    assert!(stdout.starts_with("{"));
    assert!(stdout.contains("DisplayName: \"ae\""));

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
    let doc = assert_stdout_parses(&output.stdout);
    assert_eq!(doc.get("value"), Some(HjsonValue::Integer(42)));

    fs::remove_file(temp_path).ok();
}

#[test]
fn test_edit_object_set_new_key_on_braceless_root() {
    let binary = get_binary_path();

    let test_content = "name: test\n";
    let temp_path = "/tmp/test_edit_new_braceless.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--object-set", "value", "42"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let doc = assert_stdout_parses(&output.stdout);
    assert_eq!(
        doc.get("name"),
        Some(HjsonValue::String("test".to_string()))
    );
    assert_eq!(doc.get("value"), Some(HjsonValue::Integer(42)));

    fs::remove_file(temp_path).ok();
}

#[test]
fn test_edit_object_set_new_key_does_not_reformat_whole_parent() {
    let binary = get_binary_path();

    let test_content = "Common:\n{\n    Existing: one\n    Another: two\n}\n";
    let temp_path = "/tmp/test_edit_local_object_set.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--object-set", "Common.Test", "a"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("    Existing: one"));
    assert!(stdout.contains("    Another: two"));
    assert!(stdout.contains("    Test: a"));

    fs::remove_file(temp_path).ok();
}

#[test]
fn test_edit_object_set_new_key_adds_newline_before_closing_brace() {
    let binary = get_binary_path();

    let test_content = "Common: {\n    Existing: one\n}\n";
    let temp_path = "/tmp/test_edit_object_set_closing_newline.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--object-set", "Common.TEST", "a"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("    TEST: a\n}"));

    fs::remove_file(temp_path).ok();
}

#[test]
fn test_edit_object_set_creates_nested_objects_recursively() {
    let binary = get_binary_path();

    let test_content = "Common:\n{\n    Existing: one\n}\n";
    let temp_path = "/tmp/test_edit_recursive_object_set.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--object-set", "Common.AA.Test", "a"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.as_ref(),
        "Common:\n{\n    Existing: one\n    AA: {\n        Test: a\n    }\n}\n"
    );
    let doc = assert_stdout_parses(&output.stdout);
    assert_eq!(
        doc.get("Common.AA.Test"),
        Some(HjsonValue::String("a".to_string()))
    );

    fs::remove_file(temp_path).ok();
}

#[test]
fn test_edit_object_set_recursive_creation_preserves_indentation_for_braced_child() {
    let binary = get_binary_path();

    let test_content = "Common:\n{\n    Existing: one\n}\n";
    let temp_path = "/tmp/test_edit_recursive_object_set_braced_child.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--object-set", "Common.e.TEST", "a"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.as_ref(),
        "Common:\n{\n    Existing: one\n    e: {\n        TEST: a\n    }\n}\n"
    );

    fs::remove_file(temp_path).ok();
}

#[test]
fn test_edit_object_set_recursive_creation_fails_on_non_object() {
    let binary = get_binary_path();

    let test_content = "Common:\n{\n    AA: 1\n}\n";
    let temp_path = "/tmp/test_edit_recursive_object_set_error.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--object-set", "Common.AA.Test", "a"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());

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
    let doc = assert_stdout_parses(&output.stdout);
    assert_eq!(doc.get("value"), None);
    assert_eq!(
        doc.get("name"),
        Some(HjsonValue::String("test".to_string()))
    );

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
    assert!(stdout.contains("DisplayName:"));
    assert!(stdout.contains("Tooltip: keep"));

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
    let doc = assert_stdout_parses(&output.stdout);
    assert_eq!(doc.get("items[2]"), Some(HjsonValue::Integer(3)));

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
    let doc = assert_stdout_parses(&output.stdout);
    assert_eq!(doc.get("items[0]"), Some(HjsonValue::Integer(1)));
    assert_eq!(doc.get("items[1]"), Some(HjsonValue::Integer(2)));
    assert_eq!(doc.get("items[2]"), Some(HjsonValue::Integer(3)));

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
    let doc = assert_stdout_parses(&output.stdout);
    assert_eq!(doc.get("items[0]"), Some(HjsonValue::Integer(1)));
    assert_eq!(doc.get("items[1]"), Some(HjsonValue::Integer(3)));
    assert_eq!(doc.get("items[2]"), None);

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

/// 16進数風の値は文字列として扱われることを検証
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
    let doc = assert_stdout_parses(&output.stdout);
    assert_eq!(doc.get("x"), Some(HjsonValue::String("0xFF".to_string())));

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

/// --string-style unquoted で bare string を設定できることを検証
#[test]
fn test_edit_string_style_unquoted() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let temp_path = "/tmp/test_edit_string_style_unquoted.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path,
            "--string-style",
            "unquoted",
            "--set",
            "name",
            "bareValue",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"name\": bareValue"));

    fs::remove_file(temp_path).ok();
}

/// --string-style auto で改行文字列を block string にすることを検証
#[test]
fn test_edit_string_style_auto_block() {
    let binary = get_binary_path();

    let test_content = r#"{"text": "old"}"#;
    let temp_path = "/tmp/test_edit_string_style_auto_block.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path,
            "--string-style",
            "auto",
            "--set",
            "text:str",
            "line1\nline2",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("'''\n"));
    let parsed = HjsonDocument::parse(&stdout).unwrap();
    assert_eq!(
        parsed.get("text"),
        Some(HjsonValue::String("line1\nline2".to_string()))
    );

    fs::remove_file(temp_path).ok();
}

#[test]
fn test_edit_single_quoted_literal_is_plain_string_content() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let temp_path = "/tmp/test_edit_string_style_single_literal.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args(["edit", temp_path, "--set", "name", "'single value'"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let doc = assert_stdout_parses(&output.stdout);
    assert_eq!(
        doc.get("name"),
        Some(HjsonValue::String("'single value'".to_string()))
    );

    fs::remove_file(temp_path).ok();
}

#[test]
fn test_edit_comment_append_multiline_falls_back_to_block() {
    let binary = get_binary_path();

    let test_content = r#"// header
{"name": "test"}"#;
    let temp_path = "/tmp/test_edit_comment_append_multiline.hjson";
    fs::write(temp_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            temp_path,
            "--comment-append",
            ":header",
            "line1\nline2",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("/* line1\nline2 */"));

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
    let doc = assert_stdout_parses(&output.stdout);
    assert_eq!(doc.get("value"), Some(HjsonValue::Integer(42)));

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

#[test]
fn test_edit_in_place_conflicts_with_output() {
    let binary = get_binary_path();

    let test_content = r#"{"name": "test"}"#;
    let input_path = "/tmp/test_edit_conflict_input.hjson";
    let output_path = "/tmp/test_edit_conflict_output.hjson";
    fs::write(input_path, test_content).unwrap();

    let output = Command::new(&binary)
        .args([
            "edit",
            input_path,
            "--in-place",
            "--output",
            output_path,
            "--set",
            "value",
            "42",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot be used with"));

    fs::remove_file(input_path).ok();
    fs::remove_file(output_path).ok();
}
