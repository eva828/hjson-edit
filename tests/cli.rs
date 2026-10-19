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

/// 編集結果のテキスト（標準出力）。
fn edit_stdout(args: &[&str], input: &str) -> String {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{input}").unwrap();
    file.flush().unwrap();

    let output = Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(args)
        .arg(file.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// テキストが `parse` で読めること。
fn assert_parses(text: &str) {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.flush().unwrap();
    Command::cargo_bin("hjson-edit")
        .unwrap()
        .arg("parse")
        .arg(file.path())
        .assert()
        .success();
}

/// 失敗する実行の stderr を返す。
fn edit_failure(args: &[&str], input: &str) -> (String, String) {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{input}").unwrap();
    file.flush().unwrap();

    let output = Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(args)
        .arg(file.path())
        .output()
        .unwrap();
    assert!(!output.status.success(), "expected failure: {args:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    (stdout, stderr)
}

#[test]
fn cli_edit_parses_value_as_hjson() {
    // 引用符付きは文字列、括弧は配列、エスケープもそのまま解釈される
    let stdout = edit_stdout(
        &[
            "edit",
            "--set",
            "name",
            "\"true\"",
            "--set",
            "flag",
            "true",
            "--set",
            "items",
            "[1, 2]",
            "--set",
            "msg",
            "\"a\\nb\"",
        ],
        "{\n  server: {\n    host: old\n  }\n}\n",
    );

    assert!(stdout.contains("name: \"true\""), "{stdout}");
    assert!(stdout.contains("flag: true"), "{stdout}");
    assert!(stdout.contains("items: ["), "{stdout}");
    assert!(stdout.contains("1\n    2"), "{stdout}");
    assert!(stdout.contains("msg: \"a\\nb\""), "{stdout}");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_rejects_empty_value() {
    let (stdout, stderr) = edit_failure(&["edit", "--set", "name", ""], "{a: 1}\n");
    assert!(stdout.is_empty());
    assert!(stderr.contains("Invalid value"), "{stderr}");
}

#[test]
fn cli_edit_rejects_invalid_value() {
    let (_, stderr) = edit_failure(&["edit", "--set", "name", "{"], "{a: 1}\n");
    assert!(stderr.contains("Invalid value"), "{stderr}");
}

#[test]
fn cli_edit_sets_array_item_comment() {
    let input = "{\n  a: [\n    1 // item\n    2\n  ] // keep\n}\n";
    let stdout = edit_stdout(&["edit", "--comment-set", "a[0]:inline", "changed"], input);

    // 要素のコメントだけが変わり、メンバーのコメントは残る
    assert!(stdout.contains("1 // changed"), "{stdout}");
    assert!(stdout.contains("] // keep"), "{stdout}");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_sets_container_footer_comment() {
    let input = "{\n  a: {\n    x: 1\n  }\n}\n";
    let stdout = edit_stdout(&["edit", "--comment-set", "a:footer", "note"], input);

    assert!(stdout.contains("x: 1\n    /* note */"), "{stdout}");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_removes_container_footer_comment() {
    let input = "{\n  a: {\n    x: 1\n    // note\n  }\n}\n";
    let stdout = edit_stdout(&["edit", "--comment-remove", "a:footer"], input);

    assert!(!stdout.contains("// note"), "{stdout}");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_rejects_container_footer_on_scalar() {
    let (_, stderr) = edit_failure(
        &["edit", "--comment-set", "a:footer", "note"],
        "{\n  a: 1\n}\n",
    );
    assert!(stderr.contains("Invalid target"), "{stderr}");
}

#[test]
fn cli_edit_sets_container_footer_on_single_line_container() {
    let input = "{\n  s: {a: 1}\n}\n";
    let stdout = edit_stdout(&["edit", "--comment-set", "s:footer", "note"], input);

    assert!(stdout.contains("a: 1\n    /* note */\n  }"), "{stdout}");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_sets_comment_on_scalar_document() {
    let stdout = edit_stdout(&["edit", "--comment-set", ":header", "note"], "123\n");

    assert!(stdout.starts_with("/* note */"), "{stdout}");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_sets_comment_on_last_array_element_alias() {
    let input = "{\n  items: [\n    1\n    2\n  ]\n}\n";
    let stdout = edit_stdout(&["edit", "--comment-set", "items[-]:inline", "last"], input);

    assert!(stdout.contains("2 /* last */"), "{stdout}");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_no_verify_still_edits() {
    let stdout = edit_stdout(
        &["edit", "--no-verify", "--set", "b", "hello"],
        "{\n  a: 1\n}\n",
    );
    assert!(stdout.contains("b: hello"), "{stdout}");
}

#[test]
fn cli_edit_reports_edit_errors_without_writing() {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{{\n  a: 1\n}}\n").unwrap();
    file.flush().unwrap();
    let path = file.path().to_path_buf();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["edit", "--set", "a.b", "1", "--in-place"])
        .arg(&path)
        .assert()
        .failure();

    // 失敗した編集はファイルを書き換えない
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\n  a: 1\n}\n");
}

#[test]
fn cli_edit_appends_string_and_formats_changed_container() {
    let stdout = edit_stdout(&["edit", "--set", "b", "hello"], "{a: 1}\n");
    assert_eq!(stdout, "{\n  a: 1\n  b: hello\n}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_readme_example_formats_appended_item() {
    let input = "{\n  server: {\n    host: old\n  }\n  items: [\n    a\n  ]\n}\n";
    let stdout = edit_stdout(
        &[
            "edit",
            "--set",
            "server.host",
            "localhost",
            "--set",
            "items[-]",
            "newValue",
        ],
        input,
    );
    assert_eq!(
        stdout,
        "{\n  server: {\n    host: localhost\n  }\n  items: [\n    a\n    newValue\n  ]\n}\n"
    );
    assert_parses(&stdout);
}

#[test]
fn cli_edit_keeps_detected_indent_style() {
    let stdout = edit_stdout(&["edit", "--set", "b", "hello"], "{\n    a: 1\n}\n");
    assert_eq!(stdout, "{\n    a: 1\n    b: hello\n}\n");
}

#[test]
fn cli_edit_honors_indent_width() {
    let stdout = edit_stdout(
        &["edit", "--set", "b", "hello", "--indent-width", "4"],
        "{a: 1}\n",
    );
    assert_eq!(stdout, "{\n    a: 1\n    b: hello\n}\n");
}

#[test]
fn cli_edit_in_place_writes_formatted_output() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "{{a: 1}}").unwrap();
    temp_file.flush().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["edit", "--in-place", "--set", "b", "hello"])
        .arg(&path)
        .assert()
        .success();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "{\n  a: 1\n  b: hello\n}\n");
    assert_parses(&content);
}

#[test]
fn cli_edit_creates_missing_nested_path() {
    let stdout = edit_stdout(&["edit", "--set", "a.b", "1"], "{}\n");
    assert_eq!(stdout, "{\n  a: {\n    b: 1\n  }\n}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_formats_the_container_that_received_the_insertion() {
    // 中間コンテナを生成する場合は、実在する最深の祖先（ここでは a）を整形する
    let stdout = edit_stdout(&["edit", "--set", "a.b.c", "hello"], "{\n  a: {\n  }\n}\n");
    assert_eq!(
        stdout,
        "{\n  a: {\n    b: {\n      c: hello\n    }\n  }\n}\n"
    );
    assert_parses(&stdout);
}

#[test]
fn cli_edit_string_style_applies_to_subsequent_sets() {
    let stdout = edit_stdout(
        &["edit", "--string-style", "double", "--set", "b", "hello"],
        "{a: 1}\n",
    );
    assert_eq!(stdout, "{\n  a: 1\n  b: \"hello\"\n}\n");

    let stdout = edit_stdout(
        &[
            "edit",
            "--string-style",
            "unquoted",
            "--set",
            "b",
            "hello world",
        ],
        "{a: 1}\n",
    );
    assert_eq!(stdout, "{\n  a: 1\n  b: hello world\n}\n");
}

#[test]
fn cli_edit_inline_string_style_overrides_string_style() {
    let stdout = edit_stdout(
        &["edit", "--string-style", "unquoted", "--set", "b", "\"x\""],
        "{a: 1}\n",
    );
    assert_eq!(stdout, "{\n  a: 1\n  b: \"x\"\n}\n");
}

#[test]
fn cli_edit_honors_newline_option() {
    let stdout = edit_stdout(
        &["edit", "--set", "b", "hello", "--newline", "crlf"],
        "{a: 1}\n",
    );
    assert_eq!(stdout, "{\r\n  a: 1\r\n  b: hello\r\n}\r\n");
}

#[test]
fn cli_edit_honors_indent_style_spaces_with_detected_width() {
    // --indent-style spaces のみの指定では、文書から検出した幅を保つ
    let stdout = edit_stdout(
        &["edit", "--set", "b", "hello", "--indent-style", "spaces"],
        "{\n    a: 1\n}\n",
    );
    assert_eq!(stdout, "{\n    a: 1\n    b: hello\n}\n");
}

#[test]
fn cli_edit_braceless_root_is_not_indented() {
    let stdout = edit_stdout(&["edit", "--set", "c", "three"], "a: one\nb: 2\n");
    assert_eq!(stdout, "a: one\nb: 2\nc: three\n");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_output_option_writes_formatted_file() {
    let mut input = NamedTempFile::new().unwrap();
    writeln!(input, "{{a: 1}}").unwrap();
    input.flush().unwrap();
    let output = NamedTempFile::new().unwrap();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["edit", "--set", "b", "hello", "--output"])
        .arg(output.path())
        .arg(input.path())
        .assert()
        .success();

    let content = std::fs::read_to_string(output.path()).unwrap();
    assert_eq!(content, "{\n  a: 1\n  b: hello\n}\n");
}

#[test]
fn cli_edit_formats_comments_with_the_container() {
    let stdout = edit_stdout(&["edit", "--comment-set", "a", "note"], "{\n  a: 1\n}\n");
    assert_eq!(stdout, "{\n  /* note */\n  a: 1\n}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_strips_bom_from_file() {
    use std::io::Write;

    let mut file = tempfile::NamedTempFile::new().unwrap();
    writeln!(file, "\u{FEFF}{{a: 1}}").unwrap();

    let output = assert_cmd::Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["parse", file.path().to_str().unwrap(), "--ast"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Object"),
        "BOM を除去してオブジェクトとして読む: {stdout}"
    );
}
