use hjson_edit::high_level::{
    ops::object::HjsonDocumentEdit, path::HjsonObjectPath, HjsonDocument,
};

#[test]
fn parse_error_includes_line_and_column() {
    let result = HjsonDocument::parse("{\n  \"a\": 1");
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("line") || msg.contains("column") || msg.contains("offset"),
        "error message should contain line/column info, got: {}",
        msg
    );
}

#[test]
fn path_not_found_error_includes_path() {
    let doc = HjsonDocument::parse("{\n  name: app\n}\n").unwrap();
    let result = doc.get(HjsonObjectPath::parse("missing.path").unwrap());
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("missing") || msg.contains("path"),
        "error message should contain 'missing' or 'path', got: {}",
        msg
    );
}

#[test]
fn positions_are_one_based() {
    // 3 行目の '}' で失敗する（行・列は 1 始まり）
    let error = HjsonDocument::parse("{\n  b\n}\n").unwrap_err().to_string();
    assert!(
        error.contains("line 3, column 1"),
        "error should point at line 3, column 1: {error}"
    );

    // 空でない行の途中を指す場合は列も 1 始まり
    let error = HjsonDocument::parse("{\n  \"a\" 1\n}\n")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("line 2, column 6"),
        "error should point at line 2, column 6: {error}"
    );
}

#[test]
fn skipped_input_warnings_are_one_based() {
    let document = HjsonDocument::parse("{\n  a: 1\n}\n/* x\n").unwrap();
    let warnings = document.warnings();

    assert_eq!(warnings.len(), 1, "warnings: {warnings:?}");
    assert!(
        warnings[0].contains("line 4, column 1"),
        "warning should point at line 4, column 1: {}",
        warnings[0]
    );
}

#[test]
fn columns_count_characters_not_bytes() {
    // "  名前 1" は 2 スペース + 名前（2 文字）+ スペース + 1
    let error = HjsonDocument::parse("{\n  名前 1\n}\n")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("line 2, column 5"),
        "列は文字数（バイトなら 9）: {error}"
    );

    // 絵文字（4 バイト）も 1 文字として数える
    let error = HjsonDocument::parse("{\n  絵文字😀 1\n}\n")
        .unwrap_err()
        .to_string();
    assert!(error.contains("line 2, column 7"), "列は文字数: {error}");
}

#[test]
fn a_tab_counts_as_one_column() {
    let error = HjsonDocument::parse("{\n\tabc 1\n}\n")
        .unwrap_err()
        .to_string();
    assert!(error.contains("line 2, column 5"), "タブは 1 文字: {error}");
}

#[test]
fn character_columns_do_not_change_the_line_numbers() {
    let error = HjsonDocument::parse("{\n  a: 1\n  名前 2\n}\n")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("line 3, column 5"),
        "行は 1 始まりのまま、列は文字数: {error}"
    );
}

#[test]
fn warnings_count_characters_too() {
    let document = HjsonDocument::parse("{\n  名前: 1\n}\n/* x\n").unwrap();
    let warnings = document.warnings();

    assert_eq!(warnings.len(), 1, "warnings: {warnings:?}");
    assert!(
        warnings[0].contains("line 4, column 1"),
        "警告の位置: {}",
        warnings[0]
    );
}

#[test]
fn path_not_found_prints_the_path_as_written() {
    let doc = HjsonDocument::parse("{\n  server: {\n    port: 8080\n  }\n}\n").unwrap();
    let error = doc
        .get(HjsonObjectPath::parse("server.host").unwrap())
        .unwrap_err()
        .to_string();

    assert!(error.contains("server.host"), "{error}");
    assert!(!error.contains("Key("), "{error}");
}

#[test]
fn invalid_target_prints_the_requested_path() {
    let doc = HjsonDocument::parse("a: 1\n").unwrap();
    let error = doc
        .set(HjsonObjectPath::parse("a.b").unwrap(), 2)
        .unwrap_err()
        .to_string();

    assert!(error.contains("a.b"), "{error}");
    assert!(!error.contains("Key("), "{error}");
}

#[test]
fn invalid_path_reports_the_raw_path() {
    let error = HjsonObjectPath::parse("a..b").unwrap_err().to_string();

    assert!(error.contains("a..b"), "{error}");
    assert!(error.contains("consecutive dots"), "{error}");
    assert_eq!(error.matches("Invalid path:").count(), 1, "{error}");
}

#[test]
fn invalid_comment_path_prints_the_raw_path() {
    use hjson_edit::high_level::path::HjsonCommentPath;

    let error = HjsonCommentPath::try_from("a:b").unwrap_err().to_string();

    assert!(error.contains("a:b"), "{error}");
    assert!(!error.contains("\"a:b\""), "{error}");
}
