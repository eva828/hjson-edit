use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn cli_set_multiple_values_outputs_updated_object() {
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "{{}}").unwrap();
    temp_file.flush().unwrap();
    let path = temp_file.path();

    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.arg("edit")
        .arg("--set")
        .arg("foo")
        .arg("1")
        .arg("--set")
        .arg("bar")
        .arg("2")
        .arg(path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("foo: 1"))
        .stdout(predicate::str::contains("bar: 2"));
}

#[test]
fn cli_set_and_remove_combines_operations() {
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "{{\n  foo: 1\n}}").unwrap();
    temp_file.flush().unwrap();
    let path = temp_file.path();

    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.arg("edit")
        .arg("--set")
        .arg("bar")
        .arg("2")
        .arg("--remove")
        .arg("foo")
        .arg(path);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("bar: 2"))
        .stdout(predicate::str::contains("foo: 1").not());
}

#[test]
fn cli_in_place_edit_writes_updated_file() {
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "{{\n  foo: 1\n}}").unwrap();
    temp_file.flush().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.arg("edit")
        .arg("--in-place")
        .arg("--set")
        .arg("foo")
        .arg("2")
        .arg(&path);

    cmd.assert().success();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("foo: 2"));
    assert!(!content.contains("foo: 1"));
}

#[test]
fn cli_missing_file_argument_fails_with_usage_message() {
    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.arg("edit").arg("--set").arg("foo").arg("1");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn cli_parse_error_is_reported_once_without_duplicated_prefix() {
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "{{\n  \"a\": 1\n").unwrap();
    temp_file.flush().unwrap();

    for subcommand in ["parse", "format"] {
        let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
        cmd.arg(subcommand).arg(temp_file.path());
        let assert = cmd.assert().failure();
        let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
        assert_eq!(
            stderr.matches("Parse errors:").count(),
            1,
            "stderr: {stderr}"
        );
    }
}

#[test]
fn cli_deeply_nested_input_fails_without_aborting() {
    // 深い入れ子はスタックオーバーフローで abort せず、通常のエラー終了になること
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "{}1{}", "[".repeat(20_000), "]".repeat(20_000)).unwrap();
    temp_file.flush().unwrap();

    for subcommand in ["parse", "format"] {
        let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
        cmd.arg(subcommand).arg(temp_file.path());
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Nesting too deep"));
    }
}
