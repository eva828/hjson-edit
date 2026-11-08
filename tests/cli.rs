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

/// サブコマンドの出力（`format` など、編集結果を標準出力に書くもの）。
fn format_stdout(args: &[&str], input: &str) -> String {
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
    assert!(stdout.contains("items: [1,2]"), "{stdout}");
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
fn cli_edit_appends_string_keeping_the_container_inline() {
    // 既定は保存重視。1 行で書かれたコンテナは展開しない
    let stdout = edit_stdout(&["edit", "--set", "b", "hello"], "{a: 1}\n");
    assert_eq!(stdout, "{a: 1, b: hello\n}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_format_expands_the_changed_container() {
    let stdout = edit_stdout(&["edit", "--format", "--set", "b", "hello"], "{a: 1}\n");
    assert_eq!(stdout, "{\n  a: 1\n  b: hello\n}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_keeps_blank_lines_and_inline_containers() {
    let input = "{\n  a: 1\n\n  b: {x: 1}\n}\n";
    let stdout = edit_stdout(&["edit", "--set", "a", "2"], input);

    assert_eq!(stdout, "{\n  a: 2\n\n  b: {x: 1}\n}\n");
}

#[test]
fn cli_edit_does_not_expand_inline_containers_of_the_edited_container() {
    let input = "{\n  a: 1\n  b: {x: 1}\n}\n";
    let stdout = edit_stdout(&["edit", "--set", "a", "2"], input);

    assert_eq!(stdout, "{\n  a: 2\n  b: {x: 1}\n}\n");
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
    assert_eq!(content, "{a: 1, b: hello\n}\n");
    assert_parses(&content);
}

#[test]
fn cli_edit_creates_missing_nested_path() {
    // 1 行の文書は 1 行のまま（既定は保存重視）
    let stdout = edit_stdout(&["edit", "--set", "a.b", "1"], "{}\n");
    assert_eq!(stdout, "{a: {b: 1}}\n");
    assert_parses(&stdout);

    let stdout = edit_stdout(&["edit", "--format", "--set", "a.b", "1"], "{}\n");
    assert_eq!(stdout, "{\n  a: {\n    b: 1\n  }\n}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_formats_the_container_that_received_the_insertion() {
    // 中間コンテナを生成する場合は、実在する最深の祖先（ここでは a）を整形する
    let stdout = edit_stdout(
        &["edit", "--format", "--set", "a.b.c", "hello"],
        "{\n  a: {\n  }\n}\n",
    );
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
    assert_eq!(stdout, "{a: 1, b: \"hello\"}\n");

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
    assert_eq!(stdout, "{a: 1, b: hello world\n}\n");
}

#[test]
fn cli_edit_string_style_overrides_inline_string_style() {
    let stdout = edit_stdout(
        &["edit", "--string-style", "unquoted", "--set", "b", "\"x\""],
        "{a: 1}\n",
    );
    assert_eq!(stdout, "{a: 1, b: x\n}\n");
}

#[test]
fn cli_edit_uses_inline_string_style_without_the_option() {
    let stdout = edit_stdout(&["edit", "--set", "b", "\"x\""], "{a: 1}\n");
    assert_eq!(stdout, "{a: 1, b: \"x\"}\n");
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
    assert_eq!(content, "{a: 1, b: hello\n}\n");
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

#[test]
fn cli_edit_rejects_comments_in_the_value_text() {
    let (_, stderr) = edit_failure(&["edit", "--set", "b", "{a: 1} // note"], "{\n  a: 1\n}\n");

    assert!(
        stderr.contains("comments are not allowed"),
        "stderr: {stderr}"
    );
}

#[test]
fn cli_format_ends_the_output_with_a_newline() {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{{}}").unwrap();
    file.flush().unwrap();

    let output = Command::cargo_bin("hjson-edit")
        .unwrap()
        .arg("format")
        .arg(file.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "{}\n");
}

#[test]
fn cli_reports_the_os_error_for_missing_files() {
    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args([
            "edit",
            "/nonexistent/hjson-edit-input.hjson",
            "--set",
            "a",
            "1",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"));
}

#[test]
fn cli_format_keeps_the_document_style_by_default() {
    // CRLF + 4 スペースの文書は、指定が無ければその体裁で整形する
    let stdout = format_stdout(
        &["format"],
        "{\r\n    a: 1\r\n    o: {\r\n        b: 2\r\n    }\r\n}\r\n",
    );

    assert_eq!(
        stdout,
        "{\r\n    a: 1\r\n    o: {\r\n        b: 2\r\n    }\r\n}\r\n"
    );
}

#[test]
fn cli_format_newline_option_does_not_mix_line_endings() {
    let stdout = format_stdout(
        &["format", "--newline", "lf"],
        "{\r\n  a: 1\r\n  o: {\r\n    b: 2\r\n  }\r\n}\r\n",
    );

    assert!(!stdout.contains('\r'), "CR が残っている: {stdout:?}");
    assert_eq!(stdout, "{\n  a: 1\n  o: {\n    b: 2\n  }\n}\n");
}

#[test]
fn cli_edit_with_indent_width_preserves_multiline_strings() {
    let input = "{\n  s: '''\n        deep\n  '''\n  a: 1\n}\n";
    let stdout = edit_stdout(&["edit", "--set", "a", "2", "--indent-width", "4"], input);

    // 値 "   deep" が保たれるように、本文の字下げも一緒に動く
    assert_eq!(
        stdout,
        "{\n    s: '''\n          deep\n    '''\n    a: 2\n}\n"
    );
    assert_parses(&stdout);
}

#[test]
fn cli_edit_replacing_a_block_value_formats_the_multiline_block() {
    let stdout = edit_stdout(
        &["edit", "--set", "s", "'''line1\nline2'''"],
        "{\n  s: '''x'''\n}\n",
    );

    assert_eq!(stdout, "{\n  s: '''\n    line1\n    line2\n  '''\n}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_keeps_an_inline_document_inline_when_creating_a_container() {
    // 1 行の文書に行頭でないコンテナを作っても、文書全体を展開しない
    let stdout = edit_stdout(&["edit", "--set", "b", "{x: 1}"], "{a: 1}\n");

    assert_eq!(stdout, "{a: 1, b: {x: 1}}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_does_not_expand_the_parent_of_a_new_container() {
    let stdout = edit_stdout(&["edit", "--set", "x.y", "{z: 1}"], "{a: 1, x: {b: 2}}\n");

    assert_eq!(stdout, "{a: 1, x: {b: 2, y: {z: 1}}}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_writes_block_strings_at_the_element_indent() {
    // 字下げの浅い文書でも、本文の字下げを要素に合わせて値を保つ
    let stdout = edit_stdout(&["edit", "--set", "a", "'''x\ny'''"], "{\na: 1\n}\n");

    assert_eq!(stdout, "{\na: '''\n  x\n  y\n'''\n}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_edit_keeps_multiline_blocks_inline_when_the_element_is_inline() {
    let stdout = edit_stdout(&["edit", "--set", "a", "'''x\ny'''"], "{a: 1}\n");

    assert_eq!(stdout, "{a: '''x\ny'''}\n");
    assert_parses(&stdout);
}

#[test]
fn cli_parse_reports_character_columns() {
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{{\n  名前 1\n}}\n").unwrap();
    file.flush().unwrap();

    let output = Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["parse", file.path().to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("line 2, column 5"),
        "列は文字数（バイトなら 9）: {stderr}"
    );
}

#[test]
fn cli_format_accepts_no_verify() {
    let stdout = format_stdout(&["format", "--no-verify"], "{\n  a: 1\n}\n");

    assert_eq!(stdout, "{\n  a: 1\n}\n");
}

#[test]
fn cli_format_verifies_by_default() {
    let stdout = format_stdout(&["format"], "{\n  a: 1\n}\n");

    assert_eq!(stdout, "{\n  a: 1\n}\n");
}

#[test]
fn cli_set_keeps_the_quotes_of_the_value_text() {
    let stdout = edit_stdout(&["edit", "--set", "a", "\"hi\""], "{\n  a: 1\n}\n");

    assert!(stdout.contains("a: \"hi\""), "stdout: {stdout}");
}

#[test]
fn cli_set_keeps_single_quotes_of_the_value_text() {
    let stdout = edit_stdout(&["edit", "--set", "a", "'hi'"], "{\n  a: 1\n}\n");

    assert!(stdout.contains("a: 'hi'"), "stdout: {stdout}");
}

#[test]
fn cli_string_style_overrides_the_value_text() {
    let stdout = edit_stdout(
        &["edit", "--string-style", "unquoted", "--set", "a", "\"hi\""],
        "{\n  a: 1\n}\n",
    );

    assert!(stdout.contains("a: hi"), "stdout: {stdout}");
}

#[test]
fn cli_set_accepts_an_indented_block_value_text() {
    let stdout = edit_stdout(
        &["edit", "--set", "a", "'''\n  multi\n  line'''"],
        "{\n  a: 1\n}\n",
    );

    assert!(
        stdout.contains("a: \"  multi\\n  line\""),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_set_with_an_equal_value_keeps_the_document() {
    for (value, input) in [
        ("{x: 1, y: 2}", "{\n  a: {x: 1, y: 2}\n}\n"),
        ("1.0", "{\n  a: 1.0\n}\n"),
        ("1e3", "{\n  a: 1e3\n}\n"),
    ] {
        let stdout = edit_stdout(&["edit", "--set", "a", value], input);

        assert_eq!(stdout, input, "value: {value}");
    }
}

#[test]
fn cli_format_in_place_rewrites_the_file() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "{{a:   1}}").unwrap();
    temp_file.flush().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["format", "--in-place"])
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "{\n  a: 1\n}\n");
}

#[test]
fn cli_format_output_writes_another_file() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "{{a:   1}}").unwrap();
    temp_file.flush().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.hjson");

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["format", "--output"])
        .arg(&output)
        .arg(temp_file.path())
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    assert_eq!(std::fs::read_to_string(&output).unwrap(), "{\n  a: 1\n}\n");
    assert_eq!(
        std::fs::read_to_string(temp_file.path()).unwrap(),
        "{a:   1}\n"
    );
}

#[test]
fn cli_format_in_place_and_output_conflict() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "{{a: 1}}").unwrap();
    temp_file.flush().unwrap();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["format", "--in-place", "--output", "out.hjson"])
        .arg(temp_file.path())
        .assert()
        .failure()
        .code(2);
}

#[test]
fn cli_edit_in_place_keeps_the_file_on_error() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(temp_file, "{{a: 1}}").unwrap();
    temp_file.flush().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["edit", "--in-place", "--set", "a", "[1"])
        .arg(&path)
        .assert()
        .failure();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{a: 1}\n");
}

#[test]
fn cli_edit_batch_set_and_remove() {
    let stdout = edit_stdout(
        &["edit", "--set", "a", "9", "--remove", "b"],
        "{\n  a: 1\n  b: 2\n}\n",
    );

    assert_eq!(stdout, "{\n  a: 9\n}\n");
}

#[test]
fn cli_edit_batch_set_creates_nested_paths() {
    let stdout = edit_stdout(
        &["edit", "--set", "a.b.c", "1", "--set", "d", "[1, 2]"],
        "{\n  x: 1\n}\n",
    );

    assert_eq!(stdout, "{\n  x: 1\n  a: {b: {c: 1}}\n  d: [1,2]\n}\n");
}

#[test]
fn cli_format_reads_standard_input() {
    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["format", "-"])
        .write_stdin("{a:   1}")
        .assert()
        .success()
        .stdout("{\n  a: 1\n}\n");
}

#[test]
fn cli_edit_reads_standard_input() {
    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["edit", "--set", "b", "2", "-"])
        .write_stdin("{\n  a: 1\n}\n")
        .assert()
        .success()
        .stdout("{\n  a: 1\n  b: 2\n}\n");
}

#[test]
fn cli_parse_reads_standard_input() {
    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["parse", "--simple", "-"])
        .write_stdin("{\n  a: 1\n}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Object"));
}

#[test]
fn cli_in_place_rejects_standard_input() {
    for args in [
        vec!["format", "--in-place", "-"],
        vec!["edit", "--in-place", "--set", "a", "1", "-"],
    ] {
        Command::cargo_bin("hjson-edit")
            .unwrap()
            .args(args)
            .write_stdin("{\n  a: 1\n}\n")
            .assert()
            .failure()
            .stderr(predicate::str::contains("standard input"));
    }
}

#[test]
fn cli_edit_format_keeps_the_document_footer() {
    let input = "name: app\nserver: {\n  host: localhost\n}\n// footer\n";
    let stdout = edit_stdout(&["edit", "--format", "--set", "a", "9"], input);

    assert_eq!(
        stdout,
        "name: app\nserver: {\n  host: localhost\n}\na: 9\n// footer\n"
    );
    assert_parses(&stdout);
}

#[test]
fn cli_edit_format_with_no_verify_keeps_the_document_footer() {
    let input = "name: app\nserver: {\n  host: localhost\n}\n// footer\n";
    let stdout = edit_stdout(
        &["edit", "--format", "--no-verify", "--set", "a", "9"],
        input,
    );

    assert_eq!(
        stdout,
        "name: app\nserver: {\n  host: localhost\n}\na: 9\n// footer\n"
    );
    assert_parses(&stdout);
}

#[test]
fn cli_format_keeps_the_footer_of_a_braceless_document() {
    let input = "a: 1\n// footer\n";

    assert_eq!(format_stdout(&["format"], input), input);
}

#[test]
fn cli_edit_format_indents_a_nested_container_in_a_braceless_document() {
    let input = "name: app\nserver: {\n  host: localhost\n}\n";
    let stdout = edit_stdout(&["edit", "--format", "--set", "server.port", "8080"], input);

    assert_eq!(
        stdout,
        "name: app\nserver: {\n  host: localhost\n  port: 8080\n}\n"
    );
    assert_parses(&stdout);
}

#[test]
fn cli_edit_format_verifies_the_formatting_phase() {
    // インラインコンテナのカンマ直後のコメントは整形で重複する（既知の整形バグ）。
    // 整形も検証の対象なので、--format は fail-closed で失敗する。
    // （検証を整形フェーズに戻さないとこのコマンドは成功してしまう）
    let input = "{a: 1, /* c */ b: 2}\n";
    let mut file = NamedTempFile::new().unwrap();
    write!(file, "{input}").unwrap();
    file.flush().unwrap();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["edit", "--format", "--set", "x", "1"])
        .arg(file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("comments changed"));

    // --no-verify では整形結果がそのまま出る（検証の有無で挙動が変わる）
    let stdout = edit_stdout(
        &["edit", "--format", "--no-verify", "--set", "x", "1"],
        input,
    );
    assert!(stdout.contains("/* c */"), "{stdout}");
}

#[test]
fn cli_error_messages_do_not_leak_internal_paths() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "a: 1").unwrap();
    file.flush().unwrap();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["edit", "--set", "a.b", "2"])
        .arg(file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid target a.b"))
        .stderr(predicate::str::contains("Key(").not());
}

#[test]
fn cli_invalid_path_is_not_doubled() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "a: 1").unwrap();
    file.flush().unwrap();

    Command::cargo_bin("hjson-edit")
        .unwrap()
        .args(["edit", "--set", "a..b", "1"])
        .arg(file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid path: a..b"))
        .stderr(predicate::str::contains("Invalid path:").count(1));
}
