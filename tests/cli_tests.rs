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

/// editコマンドが現時点ではプレースホルダーメッセージを返すことを検証
#[test]
fn test_cli_edit_command() {
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .args(["edit"])
        .output()
        .expect("Failed to execute command");

    // Edit command is currently a placeholder
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("placeholder"),
        "Expected placeholder message"
    );
}

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

/// 未知のコマンドを渡すと "Unknown command" エラーで終了することを検証
#[test]
fn test_cli_unknown_command() {
    let binary = get_binary_path();

    let output = Command::new(&binary)
        .args(["unknown"])
        .output()
        .expect("Failed to execute command");

    // Should fail with unknown command message
    assert!(!output.status.success(), "Should fail with unknown command");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown command"),
        "Should show unknown command message"
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
