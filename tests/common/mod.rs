//! Common test utilities for hjson-edit

use std::fs;
use std::path::PathBuf;

/// Get the path to a fixture file
pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Read a fixture file contents
pub fn read_fixture(name: &str) -> String {
    fs::read_to_string(fixture_path(name)).unwrap()
}

/// Assert that the text is valid HJSON (can be parsed)
pub fn assert_valid_hjson(text: &str) {
    hjson_edit::HjsonDocument::parse(text).unwrap();
}

/// Assert roundtrip: parse -> to_string -> reparse produces same output
pub fn assert_roundtrip(input: &str) {
    let doc = hjson_edit::HjsonDocument::parse(input).expect("input should parse");
    let rendered = doc.to_string();

    assert_eq!(
        rendered, input,
        "rendering parsed document should preserve original text"
    );

    let reparsed =
        hjson_edit::HjsonDocument::parse(&rendered).expect("rendered text should reparse");
    assert_eq!(
        reparsed.to_string(),
        rendered,
        "second render should remain stable"
    );
}

/// Get the path to the compiled CLI binary
pub fn cli_binary_path() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test executable name
    path.pop(); // Remove 'deps'
    path.push("hjson-edit");
    path
}

/// Run CLI command and return (stdout, stderr, success)
pub fn run_cli(args: &[&str]) -> (String, String, bool) {
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
