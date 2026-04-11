//! CLI integration tests
//!
//! Tests that drive the CLI `edit` sub-command are marked `#[ignore]` while the
//! AST editing operations are being rebuilt. Tests that exercise other sub-commands
//! (`parse`, `format`) and generic error handling continue to run.

use hjson_edit::{HjsonDocument, HjsonDocumentObjectEdit, HjsonValue};
use std::fs;
use std::path::PathBuf;

const COMMENT_HEAVY_INPUT: &str = r#"// This is a header comment

// {
    a: aaa,
    b: bbb
    c: cc cc
    d: dd // d
    e: /* e */ ee
    // standalone
    
f: fff
  g: ggg
    h: hhh
    // iiiiii
    i: iii
    /* j */ j:  jj
    k: {
    l: lll
      m: mm
    }
// }

// This is a footer comment 
"#;

fn cli_binary_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("hjson-edit");
    path
}

fn run_cli(args: &[&str]) -> (String, String, bool) {
    use std::process::Command;

    let output = Command::new(cli_binary_path())
        .args(args)
        .output()
        .expect("Failed to execute CLI command");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    (stdout, stderr, success)
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("hjson_test_{}.hjson", name))
}

#[test]
fn basic_commands() {
    let path = temp_path("basic");
    fs::write(&path, r#"{"key": "value"}"#).unwrap();

    let (stdout, _, success) = run_cli(&["parse", path.to_str().unwrap()]);
    assert!(success, "parse command should succeed");
    assert!(stdout.contains("Root") || stdout.contains("Object"));

    let (stdout, _, success) = run_cli(&["format", path.to_str().unwrap()]);
    assert!(success, "format command should succeed");
    assert!(stdout.contains("key"));

    fs::remove_file(&path).ok();
}

#[test]
fn parse_ast_output() {
    let path = temp_path("ast");
    fs::write(&path, r#"{"key": "value"}"#).unwrap();

    let (stdout, _, success) = run_cli(&["parse", path.to_str().unwrap(), "--ast", "--simple"]);
    assert!(success, "parse --ast should succeed");
    assert!(stdout.contains("Document"));
    assert!(stdout.contains("Object"));
    assert!(stdout.contains("Member"));
    assert!(stdout.contains("Key"));
    assert!(stdout.contains("Value"));
    assert!(stdout.contains("String"));
    assert!(!stdout.contains("Document {"));
    assert!(!stdout.contains("leading_trivia"));
    assert!(!stdout.contains("└──"));
    assert!(!stdout.contains("├──"));

    fs::remove_file(&path).ok();
}

#[test]
fn edit_preserves_comment_shape() {
    let path = temp_path("comment_shape");
    fs::write(&path, COMMENT_HEAVY_INPUT).unwrap();

    let (stdout, _, success) = run_cli(&["edit", path.to_str().unwrap()]);
    assert!(success);
    assert!(stdout.contains("// This is a header comment"));
    assert!(stdout.contains("// standalone"));
    assert!(stdout.contains("/* j */ j:  jj"));
    assert!(stdout.contains("k: {"));
    assert!(!stdout.contains("/* e */  /* e */"));
    assert!(!stdout.contains("l// This is a header comment"));

    fs::remove_file(&path).ok();
}

#[test]
fn edit_set_preserves_footer_comment() {
    let path = temp_path("footer_comment");
    fs::write(&path, COMMENT_HEAVY_INPUT).unwrap();

    let (stdout, _, success) = run_cli(&["edit", path.to_str().unwrap(), "--set", "aa", "aaa"]);
    assert!(success);
    assert!(stdout.contains("// }"));
    assert!(stdout.contains("// This is a footer comment"));
    assert!(stdout.contains("aa: aaa"));

    fs::remove_file(&path).ok();
}

#[test]
fn edit_remove_preserves_independent_object_comment() {
    let path = temp_path("remove_object_comment");
    fs::write(&path, "{\n  a: 1\n\n  // keep me\n\n  b: 2\n}\n").unwrap();

    let (stdout, _, success) = run_cli(&["edit", path.to_str().unwrap(), "--remove", "b"]);
    assert!(success);
    assert!(stdout.contains("// keep me"));
    assert!(stdout.contains("a: 1"));
    assert!(!stdout.contains("b: 2"));

    fs::remove_file(&path).ok();
}

#[test]
fn edit_remove_preserves_independent_array_comment() {
    let path = temp_path("remove_array_comment");
    fs::write(&path, "[\n  1\n\n  // keep me\n\n  2\n]\n").unwrap();

    let (stdout, _, success) = run_cli(&["edit", path.to_str().unwrap(), "--remove", "[1]"]);
    assert!(success);
    assert!(stdout.contains("// keep me"));
    assert!(stdout.contains("1"));
    assert!(!stdout.contains("2"));

    fs::remove_file(&path).ok();
}

#[test]
fn edit_comment_remove_preserves_independent_block() {
    let path = temp_path("remove_comment_block");
    fs::write(&path, "{\n  // keep me\n\n  name: value\n}\n").unwrap();

    let (stdout, _, success) = run_cli(&[
        "edit",
        path.to_str().unwrap(),
        "--comment-remove",
        "name:before",
    ]);
    assert!(success);
    assert!(stdout.contains("// keep me"));
    assert!(stdout.contains("name: value"));

    fs::remove_file(&path).ok();
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn edit_object_set_and_remove() {
    let path = temp_path("object");
    fs::write(&path, r#"{"name": "test"}"#).unwrap();

    let (stdout, _, success) =
        run_cli(&["edit", path.to_str().unwrap(), "--set", "name", "updated"]);
    assert!(success);
    assert!(stdout.contains("updated"));
    assert!(!stdout.contains("test"));

    let (stdout, _, success) = run_cli(&["edit", path.to_str().unwrap(), "--set", "new_key", "42"]);
    assert!(success);
    assert!(stdout.contains("new_key"));

    let (stdout, _, success) = run_cli(&["edit", path.to_str().unwrap(), "--remove", "name"]);
    assert!(success);
    assert!(!stdout.contains("name"));

    fs::remove_file(&path).ok();
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn edit_array_operations() {
    let path = temp_path("array");
    fs::write(&path, r#"{"items": [1, 2]}"#).unwrap();

    // array-append → set with items[-] path
    let (stdout, _, success) = run_cli(&["edit", path.to_str().unwrap(), "--set", "items[-]", "3"]);
    assert!(success);
    let doc = hjson_edit::HjsonDocument::parse(&stdout).unwrap();
    assert_eq!(
        doc.get("items[2]").unwrap(),
        hjson_edit::HjsonValue::Integer(3)
    );

    // array-remove → remove with items[0] path
    let (stdout, _, success) = run_cli(&["edit", path.to_str().unwrap(), "--remove", "items[0]"]);
    assert!(success);
    let doc = hjson_edit::HjsonDocument::parse(&stdout).unwrap();
    assert_eq!(
        doc.get("items[0]").unwrap(),
        hjson_edit::HjsonValue::Integer(2)
    );

    fs::remove_file(&path).ok();
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn edit_comment_operations() {
    let path = temp_path("comment");
    fs::write(&path, r#"{"key": "value"}"#).unwrap();

    let (stdout, _, success) = run_cli(&[
        "edit",
        path.to_str().unwrap(),
        "--comment-set",
        ":header",
        "Config",
    ]);
    assert!(success);
    assert!(stdout.contains("Config"));

    fs::remove_file(&path).ok();
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn edit_preserves_crlf_when_writing() {
    let path = temp_path("crlf_edit");
    fs::write(&path, "{\r\n  key: value\r\n}\r\n").unwrap();

    let (_, _, success) = run_cli(&[
        "edit",
        path.to_str().unwrap(),
        "--in-place",
        "--set",
        "added",
        "1",
    ]);
    assert!(success);

    let output = fs::read_to_string(&path).unwrap();
    assert_eq!(output, "{\r\n  key: value\r\n  added: 1\r\n}\r\n");

    fs::remove_file(&path).ok();
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn edit_type_inference() {
    let path = temp_path("types");
    fs::write(&path, "{}").unwrap();

    let test_cases = [
        ("42", HjsonValue::Integer(42)),
        ("3.14", HjsonValue::Float("3.14".parse().unwrap())),
        ("true", HjsonValue::Bool(true)),
        ("null", HjsonValue::Null),
    ];

    for (input, expected) in test_cases {
        let (stdout, _, success) =
            run_cli(&["edit", path.to_str().unwrap(), "--set", "key", input]);
        assert!(success, "Type inference should work for: {}", input);

        let doc = HjsonDocument::parse(&stdout).unwrap();
        assert_eq!(
            doc.get("key").unwrap(),
            expected,
            "Type mismatch for: {}",
            input
        );
    }

    fs::remove_file(&path).ok();
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn edit_output_options() {
    let input_path = temp_path("input");
    let output_path = temp_path("output");
    fs::write(&input_path, r#"{"key": "value"}"#).unwrap();

    let (_, _, success) = run_cli(&[
        "edit",
        input_path.to_str().unwrap(),
        "--output",
        output_path.to_str().unwrap(),
        "--set",
        "new",
        "added",
    ]);
    assert!(success);

    let original = fs::read_to_string(&input_path).unwrap();
    assert!(!original.contains("new"));

    let output = fs::read_to_string(&output_path).unwrap();
    assert!(output.contains("new"));

    fs::remove_file(&input_path).ok();
    fs::remove_file(&output_path).ok();
}

#[test]
fn error_cases() {
    let (_, stderr, success) = run_cli(&[]);
    assert!(!success);
    assert!(stderr.contains("Usage") || stderr.contains("required"));

    let (_, stderr, success) = run_cli(&["unknown"]);
    assert!(!success);
    assert!(stderr.contains("unrecognized") || stderr.contains("unknown"));

    let (_, stderr, success) = run_cli(&["parse", "/nonexistent/file.hjson"]);
    assert!(!success);
    assert!(stderr.contains("read") || stderr.contains("file"));

    let path = temp_path("invalid");
    fs::write(&path, "{invalid").unwrap();
    let (_, stderr, success) = run_cli(&["parse", path.to_str().unwrap()]);
    assert!(!success);
    assert!(stderr.contains("Parse") || stderr.contains("error"));
    fs::remove_file(&path).ok();
}

#[test]
#[ignore = "AST operations not yet implemented"]
fn preserves_comments() {
    let path = temp_path("preserve");
    fs::write(
        &path,
        r#"// Header
{
  // Key comment
  name: "test"
}"#,
    )
    .unwrap();

    let (stdout, _, success) = run_cli(&["edit", path.to_str().unwrap(), "--set", "value", "42"]);
    assert!(success);

    assert!(stdout.contains("Header"));
    assert!(stdout.contains("Key comment"));

    fs::remove_file(&path).ok();
}
