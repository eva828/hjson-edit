//! `HjsonDocument::format_slots`（複数スロットの一括整形）。

use hjson_edit::high_level::ops::object::HjsonDocumentEdit;
use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::{HjsonDocument, HjsonEditError, HjsonValue};

fn path(raw: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(raw).unwrap()
}

/// 複数行になった Block 文字列を持つ文書（スロット整形の対象）。
fn document_with_multiline_blocks() -> HjsonDocument {
    HjsonDocument::parse("{\n  s: '''x'''\n  t: '''y'''\n  a: 1\n}\n")
        .unwrap()
        .set(path("s"), HjsonValue::String("line1\nline2".to_string()))
        .unwrap()
        .set(path("t"), HjsonValue::String("line3\nline4".to_string()))
        .unwrap()
}

#[test]
fn formats_several_slots_at_once() {
    let document = document_with_multiline_blocks();
    let config = document.current_format_config();

    let formatted = document
        .format_slots(&[path("s"), path("t")], &config)
        .unwrap();

    assert_eq!(
        formatted.to_string(),
        "{\n  s: '''\n    line1\n    line2\n  '''\n  t: '''\n    line3\n    line4\n  '''\n  a: 1\n}\n"
    );
    // 1 つずつ整えた結果と同じ
    let one_by_one = document
        .format_slot(&path("s"), &config)
        .unwrap()
        .format_slot(&path("t"), &config)
        .unwrap();
    assert_eq!(formatted.to_string(), one_by_one.to_string());
}

#[test]
fn a_repeated_path_is_formatted_once() {
    let document = document_with_multiline_blocks();
    let config = document.current_format_config();

    let formatted = document
        .format_slots(&[path("s"), path("s")], &config)
        .unwrap();

    assert_eq!(
        formatted.to_string(),
        "{\n  s: '''\n    line1\n    line2\n  '''\n  t: '''line3\nline4'''\n  a: 1\n}\n"
    );
}

#[test]
fn an_empty_path_is_rejected() {
    let document = document_with_multiline_blocks();
    let error = document
        .format_slots(&[path("")], &document.current_format_config())
        .unwrap_err();

    assert!(matches!(error, HjsonEditError::InvalidPath(_)), "{error:?}");
}

#[test]
fn a_missing_path_is_reported() {
    let document = document_with_multiline_blocks();
    let error = document
        .format_slots(&[path("nosuch")], &document.current_format_config())
        .unwrap_err();

    assert!(
        matches!(error, HjsonEditError::PathNotFound(ref path) if path.to_string() == "nosuch"),
        "{error:?}"
    );
}

#[test]
fn formatting_several_slots_keeps_the_values() {
    let document = document_with_multiline_blocks();
    let config = document.current_format_config();

    let formatted = document
        .format_slots(&[path("s"), path("t")], &config)
        .unwrap();

    assert_eq!(
        formatted.get(path("s")).unwrap(),
        HjsonValue::String("line1\nline2".to_string())
    );
    assert_eq!(
        formatted.get(path("t")).unwrap(),
        HjsonValue::String("line3\nline4".to_string())
    );
    assert_eq!(formatted.all_comments(), document.all_comments());
}
