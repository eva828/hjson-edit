use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

use assert_cmd::Command;
use predicates::prelude::*;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn create_temp_file(content: &str) -> String {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut temp_path = std::env::temp_dir();
    temp_path.push(format!(
        "hjson_edit_test_{}_{}.hjson",
        std::process::id(),
        n
    ));
    let mut file = File::create(&temp_path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    temp_path.to_string_lossy().to_string()
}

struct TempFileGuard(String);

impl TempFileGuard {
    fn new(path: &str) -> Self {
        TempFileGuard(path.to_string())
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn cli_parse_basic_file() {
    let path = create_temp_file("{\n  name: app\n}\n");
    let _guard = TempFileGuard::new(&path);
    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.args(["parse", &path]);
    cmd.assert().success();
}

#[test]
fn cli_parse_invalid_hjson_fails() {
    let path = create_temp_file("{\"a\": 1");
    let _guard = TempFileGuard::new(&path);
    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.args(["parse", &path]);
    cmd.assert().failure();
}

#[test]
fn cli_format_basic_file() {
    let path = create_temp_file("{\n  name: app\n}\n");
    let _guard = TempFileGuard::new(&path);
    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.args(["format", &path]);
    cmd.assert().success();
}

#[test]
fn cli_edit_set_value() {
    let path = create_temp_file("{\n  name: app\n}\n");
    let _guard = TempFileGuard::new(&path);
    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.args(["edit", "--set", "name", "updated", &path]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("updated"));
}

#[test]
fn cli_edit_remove_value() {
    let path = create_temp_file("{\n  name: app\n}\n");
    let _guard = TempFileGuard::new(&path);
    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.args(["edit", "--remove", "name", &path]);
    cmd.assert().success();
}

#[test]
fn cli_edit_comment_set_header() {
    let path = create_temp_file("{\n  name: app\n}\n");
    let _guard = TempFileGuard::new(&path);
    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.args(["edit", "--comment-set", ":header", "header text", &path]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("header text"));
}

#[test]
fn cli_edit_comment_remove_header() {
    let path = create_temp_file("// header\n{\n  name: app\n}\n");
    let _guard = TempFileGuard::new(&path);
    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.args(["edit", "--comment-remove", ":header", &path]);
    cmd.assert().success();
}

#[test]
fn cli_edit_missing_file_fails() {
    let mut cmd = Command::cargo_bin("hjson-edit").unwrap();
    cmd.args(["parse", "/nonexistent/path.hjson"]);
    cmd.assert().failure();
}
