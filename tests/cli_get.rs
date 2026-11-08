//! `get` コマンド（値を JSON で読む）。

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

const DOCUMENT: &str = r#"{
  name: "app",
  port: 8080,
  ratio: 1.5,
  flag: true,
  nothing: null,
  items: [1, [2, 3], {k: "v"}],
  empty_object: {},
  empty_array: [],
  "a\"b": "x\ny\tz",
  uni: "日本語"
}
"#;

fn get_stdout(args: &[&str], input: &str) -> String {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{input}").unwrap();
    file.flush().unwrap();

    let output = Command::cargo_bin("hjson-edit")
        .unwrap()
        .arg("get")
        .arg(file.path())
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap()
}

fn get_with_document(path: &str) -> String {
    let paths = [path];
    get_stdout(&paths, DOCUMENT)
}

#[test]
fn get_prints_scalars_as_json() {
    assert_eq!(get_with_document("port"), "8080\n");
    assert_eq!(get_with_document("ratio"), "1.5\n");
    assert_eq!(get_with_document("flag"), "true\n");
    assert_eq!(get_with_document("nothing"), "null\n");
}

#[test]
fn get_prints_a_string_with_quotes_and_raw_without() {
    assert_eq!(get_with_document("name"), "\"app\"\n");
    assert_eq!(get_stdout(&["--raw", "name"], DOCUMENT), "app\n");
}

#[test]
fn get_prints_nested_paths() {
    assert_eq!(get_with_document("items[2].k"), "\"v\"\n");
    assert_eq!(get_with_document("items[1][0]"), "2\n");
}

#[test]
fn get_prints_containers_as_json() {
    assert_eq!(
        get_with_document("items"),
        "[\n  1,\n  [\n    2,\n    3\n  ],\n  {\n    \"k\": \"v\"\n  }\n]\n"
    );
    assert_eq!(get_with_document("empty_object"), "{}\n");
    assert_eq!(get_with_document("empty_array"), "[]\n");
}

#[test]
fn get_escapes_json_strings() {
    assert_eq!(get_with_document("a\"b"), "\"x\\ny\\tz\"\n");
    assert_eq!(get_with_document("uni"), "\"日本語\"\n");
}

#[test]
fn get_reports_a_missing_path() {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{DOCUMENT}").unwrap();
    file.flush().unwrap();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .arg("get")
        .arg(file.path())
        .arg("items[9]")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Path not found: items[9]"));
}

#[test]
fn get_raw_rejects_a_non_string() {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{DOCUMENT}").unwrap();
    file.flush().unwrap();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .arg("get")
        .arg(file.path())
        .args(["--raw", "port"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid target port"));
}

#[test]
fn get_prints_the_whole_document_for_an_empty_path() {
    assert_eq!(get_stdout(&[""], "{\n  a: 1\n}\n"), "{\n  \"a\": 1\n}\n");
}

#[test]
fn get_rejects_a_non_finite_number() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "a: 1e999").unwrap();
    file.flush().unwrap();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .arg("get")
        .arg(file.path())
        .arg("a")
        .assert()
        .failure()
        .stderr(predicate::str::contains("non-finite"));
}

#[test]
fn get_reads_standard_input() {
    let output = Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["get", "-", "server.port"])
        .write_stdin("server: {\n  port: 8080\n}\n")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "8080\n");
}
