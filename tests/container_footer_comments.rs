//! コンテナ内部の footer コメント（閉じ区切りの直前のコメント）。
//!
//! `<path>:footer` でコンテナの末尾コメントを読み書きする。文書全体の末尾は
//! `:footer`（パスなし）で、両者は別のコメントとして扱う。

use hjson_edit::{
    high_level::{
        ops::{comment::HjsonDocumentCommentEdit, object::HjsonDocumentEdit},
        path::{HjsonCommentPath, HjsonObjectPath},
    },
    HjsonDocument, HjsonEditError,
};

fn comment_path(raw: &str) -> HjsonCommentPath {
    HjsonCommentPath::try_from(raw).expect("valid comment path")
}

fn object_path(raw: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(raw.to_string()).expect("valid path")
}

const NESTED: &str = "{\n  a: {\n    x: 1\n    // inner\n  }\n}\n";

#[test]
fn get_container_footer_comment() {
    let doc = HjsonDocument::parse(NESTED).unwrap();

    assert_eq!(
        doc.get_comment(comment_path("a:footer")).unwrap(),
        Some("inner".to_string())
    );
    // コメントの無いコンテナは None
    let doc = HjsonDocument::parse("{\n  a: {\n    x: 1\n  }\n}\n").unwrap();
    assert_eq!(doc.get_comment(comment_path("a:footer")).unwrap(), None);
}

#[test]
fn get_container_footer_of_array() {
    let input = "{\n  a: [\n    1\n    // items\n  ]\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();

    assert_eq!(
        doc.get_comment(comment_path("a:footer")).unwrap(),
        Some("items".to_string())
    );
}

#[test]
fn container_footer_is_distinct_from_document_footer() {
    let input = "{\n  a: {\n    x: 1\n    // inner\n  }\n}\n// document\n";
    let doc = HjsonDocument::parse(input).unwrap();

    assert_eq!(
        doc.get_comment(comment_path("a:footer")).unwrap(),
        Some("inner".to_string())
    );
    assert_eq!(
        doc.get_comment(HjsonCommentPath::Footer).unwrap(),
        Some("document".to_string())
    );
}

#[test]
fn set_container_footer_comment_adds_a_line() {
    let doc = HjsonDocument::parse("{\n  a: {\n    x: 1\n  }\n}\n").unwrap();
    let updated = doc.set_comment(comment_path("a:footer"), "note").unwrap();

    assert_eq!(
        updated.to_string(),
        "{\n  a: {\n    x: 1\n    /* note */\n  }\n}\n"
    );
}

#[test]
fn set_container_footer_comment_replaces_existing() {
    let doc = HjsonDocument::parse("{\n  a: {\n    x: 1\n    /* old */\n  }\n}\n").unwrap();
    let updated = doc.set_comment(comment_path("a:footer"), "new").unwrap();

    assert_eq!(
        updated.to_string(),
        "{\n  a: {\n    x: 1\n    /* new */\n  }\n}\n"
    );
}

#[test]
fn set_container_footer_comment_keeps_the_style() {
    let input = "{\n  a: {\n    x: 1\n    // old\n  }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set_comment(comment_path("a:footer"), "new").unwrap();

    assert_eq!(
        updated.to_string(),
        "{\n  a: {\n    x: 1\n    // new\n  }\n}\n"
    );
}

#[test]
fn remove_container_footer_comment() {
    let doc = HjsonDocument::parse(NESTED).unwrap();
    let updated = doc.remove_comment(comment_path("a:footer")).unwrap();

    assert_eq!(updated.to_string(), "{\n  a: {\n    x: 1\n  }\n}\n");
}

#[test]
fn remove_missing_container_footer_is_noop() {
    let input = "{\n  a: {\n    x: 1\n  }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.remove_comment(comment_path("a:footer")).unwrap();

    assert_eq!(updated.to_string(), input);
}

#[test]
fn append_container_footer_comment() {
    let doc = HjsonDocument::parse(NESTED).unwrap();
    let updated = doc
        .append_comment(comment_path("a:footer"), "more")
        .unwrap();

    assert_eq!(
        updated.get_comment(comment_path("a:footer")).unwrap(),
        Some("inner\nmore".to_string())
    );
}

#[test]
fn set_container_footer_on_empty_container_indents_like_elements() {
    let doc = HjsonDocument::parse("{\n  a: {\n  }\n}\n").unwrap();
    let updated = doc.set_comment(comment_path("a:footer"), "note").unwrap();

    // 閉じ区切りの字下げに 1 段足した位置に入る
    assert_eq!(updated.to_string(), "{\n  a: {\n    /* note */\n  }\n}\n");
}

#[test]
fn container_footer_on_tabs_uses_tabs() {
    let input = "{\n\ta: {\n\t\tx: 1\n\t}\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set_comment(comment_path("a:footer"), "note").unwrap();

    assert_eq!(
        updated.to_string(),
        "{\n\ta: {\n\t\tx: 1\n\t\t/* note */\n\t}\n}\n"
    );
}

#[test]
fn container_footer_on_scalar_is_rejected() {
    let doc = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();
    let error = doc
        .set_comment(comment_path("a:footer"), "note")
        .unwrap_err();

    assert!(
        matches!(error, HjsonEditError::InvalidTarget { .. }),
        "unexpected error: {error:?}"
    );
}

#[test]
fn container_footer_on_scalar_path_is_rejected() {
    // 閉じ区切りを持たない（値がスカラー）場合はコンテナ footer を作れない
    let doc = HjsonDocument::parse("a: 1\n").unwrap();
    let error = doc
        .set_comment(comment_path("a:footer"), "note")
        .unwrap_err();

    assert!(
        matches!(error, HjsonEditError::InvalidTarget { .. }),
        "unexpected error: {error:?}"
    );
}

#[test]
fn container_footer_of_the_document_root_uses_a_dot_path() {
    let input = "{\n  a: 1\n  // inside\n}\n// outside\n";
    let doc = HjsonDocument::parse(input).unwrap();

    assert_eq!(
        doc.get_comment(comment_path(".:footer")).unwrap(),
        Some("inside".to_string())
    );
    assert_eq!(
        doc.get_comment(comment_path(":footer")).unwrap(),
        Some("outside".to_string())
    );

    let updated = doc.set_comment(comment_path(".:footer"), "new").unwrap();
    assert_eq!(updated.to_string(), "{\n  a: 1\n  // new\n}\n// outside\n");
    assert_eq!(
        updated.get_comment(comment_path(":footer")).unwrap(),
        Some("outside".to_string())
    );
}

#[test]
fn container_footer_of_the_document_root_is_rejected_when_braceless() {
    let doc = HjsonDocument::parse("a: 1\n").unwrap();
    let error = doc
        .set_comment(comment_path(".:footer"), "note")
        .unwrap_err();

    assert!(
        matches!(error, HjsonEditError::InvalidTarget { .. }),
        "unexpected error: {error:?}"
    );
}

#[test]
fn comment_paths_round_trip_through_text() {
    for raw in [
        ":header", ":footer", ".:footer", "a:before", "a:inline", "a:footer",
    ] {
        let path = comment_path(raw);
        assert_eq!(comment_path(&path.to_string()), path);
    }

    // 文書直下は `.` で表す
    let root = HjsonObjectPath::from_segments(Vec::new());
    assert_eq!(
        HjsonCommentPath::ContainerFooter(root).to_string(),
        ".:footer"
    );
}

#[test]
fn container_footer_for_missing_path_is_not_found() {
    let doc = HjsonDocument::parse(NESTED).unwrap();
    let error = doc.get_comment(comment_path("missing:footer")).unwrap_err();

    assert!(matches!(error, HjsonEditError::PathNotFound(_)));
}

#[test]
fn container_footer_is_separate_from_element_comments() {
    let input = "{\n  a: {\n    x: 1 // inline\n    // footer\n  }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();

    assert_eq!(
        doc.get_comment(comment_path("a.x:inline")).unwrap(),
        Some("inline".to_string())
    );
    assert_eq!(
        doc.get_comment(comment_path("a:footer")).unwrap(),
        Some("footer".to_string())
    );

    // 片方を消しても他方は残る
    let updated = doc
        .clone()
        .remove_comment(comment_path("a:footer"))
        .unwrap();
    assert_eq!(
        updated.get_comment(comment_path("a.x:inline")).unwrap(),
        Some("inline".to_string())
    );
}

#[test]
fn container_footer_values_are_unchanged() {
    let doc = HjsonDocument::parse(NESTED).unwrap();
    let before = doc.get(object_path("a.x")).unwrap();

    let updated = doc.set_comment(comment_path("a:footer"), "note").unwrap();

    assert_eq!(updated.get(object_path("a.x")).unwrap(), before);
}

#[test]
fn container_footer_breaks_the_line_of_a_single_line_container() {
    let input = "{\n  s: {a: 1}\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set_comment(comment_path("s:footer"), "note").unwrap();

    assert_eq!(
        updated.to_string(),
        "{\n  s: {a: 1\n    /* note */\n  }\n}\n"
    );
    assert_eq!(
        updated.get_comment(comment_path("s:footer")).unwrap(),
        Some("note".to_string())
    );
}

#[test]
fn container_footer_of_a_nested_single_line_document() {
    let input = "{\"s\": {\"a\": 1}}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set_comment(comment_path("s:footer"), "note").unwrap();

    assert_eq!(
        updated.get_comment(comment_path("s:footer")).unwrap(),
        Some("note".to_string())
    );
}

#[test]
fn container_footer_accepts_the_last_element_alias() {
    // `[-]` は末尾要素を指すため、その要素のコンテナ footer として解決される
    let input = "{\n  a: [\n    {\n      x: 1\n    }\n  ]\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set_comment(comment_path("a[-]:footer"), "note")
        .unwrap();

    assert_eq!(
        updated.get_comment(comment_path("a[0]:footer")).unwrap(),
        Some("note".to_string())
    );
}

#[test]
fn empty_container_footer_uses_one_indent_level() {
    for (input, expected) in [
        ("{\n  a: {}\n}\n", "{\n  a: {\n    /* note */\n  }\n}\n"),
        ("{\n  a: { }\n}\n", "{\n  a: {\n    /* note */\n  }\n}\n"),
    ] {
        let doc = HjsonDocument::parse(input).unwrap();
        let updated = doc.set_comment(comment_path("a:footer"), "note").unwrap();

        assert_eq!(updated.to_string(), expected);
    }
}

#[test]
fn removing_the_only_comment_of_a_container_keeps_the_delimiters() {
    let input = "{\n  a: {\n    /* note */\n  }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.remove_comment(comment_path("a:footer")).unwrap();

    assert_eq!(updated.to_string(), "{\n  a: {\n  }\n}\n");
    assert_eq!(updated.get_comment(comment_path("a:footer")).unwrap(), None);
}

#[test]
fn removing_a_missing_container_footer_changes_nothing() {
    for input in [
        "{\n  s: {a: 1}\n}\n",
        "{\n  s: {a: 1 }\n}\n",
        "{\n  s: {\n    a: 1}\n}\n",
    ] {
        let doc = HjsonDocument::parse(input).unwrap();
        let updated = doc.remove_comment(comment_path("s:footer")).unwrap();

        assert_eq!(updated.to_string(), input);
    }
}

#[test]
fn container_footer_keeps_the_original_closing_indent() {
    let input = "{\n  a: {\n    x: 1\n    // c\n      }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set_comment(comment_path("a:footer"), "note").unwrap();

    assert_eq!(
        updated.to_string(),
        "{\n  a: {\n    x: 1\n    // note\n      }\n}\n"
    );
}

#[test]
fn container_footer_keeps_inline_comments_of_the_last_element() {
    let input = "{\n  s: {x: 1} // host\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set_comment(comment_path("s:footer"), "note").unwrap();

    // 行末コメントは s の値（コンテナ）に付いたまま
    assert_eq!(
        updated.get_comment(comment_path("s:inline")).unwrap(),
        Some("host".to_string())
    );
    assert_eq!(
        updated.get_comment(comment_path("s:footer")).unwrap(),
        Some("note".to_string())
    );
}

#[test]
fn container_footer_after_a_trailing_comma() {
    let input = "{\n  a: {\n    x: 1,\n  }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set_comment(comment_path("a:footer"), "note").unwrap();

    assert_eq!(
        updated.to_string(),
        "{\n  a: {\n    x: 1,\n    /* note */\n  }\n}\n"
    );
    assert_eq!(
        updated.get(object_path("a.x")).unwrap(),
        hjson_edit::high_level::value::HjsonValue::Integer(1)
    );
}

#[test]
fn container_footer_of_array_with_tabs() {
    let input = "{\n\ta: [\n\t\t1\n\t]\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set_comment(comment_path("a:footer"), "note").unwrap();

    assert_eq!(
        updated.to_string(),
        "{\n\ta: [\n\t\t1\n\t\t/* note */\n\t]\n}\n"
    );
}

#[test]
fn comment_path_aliases_resolve_inside_nested_paths() {
    let input = "{\n  items: [\n    {\n      x: 1\n    }\n  ]\n}\n";

    for alias in ["items[-].x", "items[+0].x"] {
        let doc = HjsonDocument::parse(input).unwrap();
        let updated = doc
            .set_comment(comment_path(&format!("{alias}:before")), "note")
            .unwrap();

        assert_eq!(
            updated
                .get_comment(comment_path("items[0].x:before"))
                .unwrap(),
            Some("note".to_string())
        );
    }
}
