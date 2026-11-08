//! 配列要素へのコメント操作。
//!
//! 要素自身のコメントを読み書きし、親メンバーのコメントには触れないことを確認する。

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

const ARRAY_IN_OBJECT: &str = "{\n  a: [\n    1 // item\n    2\n  ] // keep\n}\n";

#[test]
fn get_inline_comment_of_array_item() {
    let doc = HjsonDocument::parse(ARRAY_IN_OBJECT).unwrap();

    assert_eq!(
        doc.get_comment(comment_path("a[0]:inline")).unwrap(),
        Some("item".to_string())
    );
    // コメントの無い要素は None
    assert_eq!(doc.get_comment(comment_path("a[1]:inline")).unwrap(), None);
    // 親メンバーのコメントは要素のコメントに化けない
    assert_eq!(
        doc.get_comment(comment_path("a:inline")).unwrap(),
        Some("keep".to_string())
    );
}

#[test]
fn set_inline_comment_of_array_item_keeps_member_comment() {
    let doc = HjsonDocument::parse(ARRAY_IN_OBJECT).unwrap();
    let updated = doc
        .set_comment(comment_path("a[1]:inline"), "second")
        .unwrap();

    assert_eq!(
        updated.to_string(),
        "{\n  a: [\n    1 // item\n    2 // second\n  ] // keep\n}\n"
    );
    assert_eq!(
        updated.get_comment(comment_path("a[0]:inline")).unwrap(),
        Some("item".to_string())
    );
    assert_eq!(
        updated.get_comment(comment_path("a:inline")).unwrap(),
        Some("keep".to_string())
    );
}

#[test]
fn remove_inline_comment_of_array_item_keeps_member_comment() {
    let doc = HjsonDocument::parse(ARRAY_IN_OBJECT).unwrap();
    let updated = doc.remove_comment(comment_path("a[0]:inline")).unwrap();

    assert_eq!(
        updated.to_string(),
        "{\n  a: [\n    1\n    2\n  ] // keep\n}\n"
    );
    assert_eq!(
        updated.get_comment(comment_path("a[0]:inline")).unwrap(),
        None
    );
    assert_eq!(
        updated.get_comment(comment_path("a:inline")).unwrap(),
        Some("keep".to_string())
    );
}

#[test]
fn set_before_comment_of_array_item() {
    let doc = HjsonDocument::parse(ARRAY_IN_OBJECT).unwrap();
    let updated = doc
        .set_comment(comment_path("a[0]:before"), "note")
        .unwrap();

    // 要素の行の前にコメント行が入る（要素自身の行末コメントは残る）
    assert_eq!(
        updated.to_string(),
        "{\n  a: [\n    // note\n    1 // item\n    2\n  ] // keep\n}\n"
    );
}

#[test]
fn append_comment_of_array_item() {
    let doc = HjsonDocument::parse(ARRAY_IN_OBJECT).unwrap();
    let updated = doc
        .append_comment(comment_path("a[0]:inline"), "more")
        .unwrap();

    assert_eq!(
        updated.get_comment(comment_path("a[0]:inline")).unwrap(),
        Some("item\nmore".to_string())
    );
}

#[test]
fn array_item_comments_in_root_array() {
    let doc = HjsonDocument::parse("[\n  1 // item\n  2\n]\n").unwrap();

    assert_eq!(
        doc.get_comment(comment_path("[0]:inline")).unwrap(),
        Some("item".to_string())
    );

    let updated = doc.set_comment(comment_path("[1]:before"), "note").unwrap();
    assert_eq!(updated.to_string(), "[\n  1 // item\n  // note\n  2\n]\n");
}

#[test]
fn inline_comment_on_unquoted_string_is_rejected() {
    // 引用符なし文字列は行末までを飲み込むため、直後にコメントを置けない
    let doc = HjsonDocument::parse("[\n  value\n]\n").unwrap();
    let error = doc
        .set_comment(comment_path("[0]:inline"), "note")
        .unwrap_err();

    assert!(
        matches!(error, HjsonEditError::InvalidTarget { .. }),
        "unexpected error: {error:?}"
    );
}

#[test]
fn comment_path_for_missing_element_is_not_found() {
    let doc = HjsonDocument::parse(ARRAY_IN_OBJECT).unwrap();
    let error = doc.get_comment(comment_path("a[9]:inline")).unwrap_err();

    assert!(matches!(error, HjsonEditError::PathNotFound(_)));
}

#[test]
fn comment_path_for_document_body_is_rejected() {
    let doc = HjsonDocument::parse(ARRAY_IN_OBJECT).unwrap();
    let error = doc.get_comment(comment_path(":inline")).unwrap_err();

    assert!(matches!(error, HjsonEditError::InvalidTarget { .. }));
}

#[test]
fn set_comment_on_member_inside_array_item() {
    let doc =
        HjsonDocument::parse("{\n  items: [\n    {\n      name: \"a\"\n    }\n  ]\n}\n").unwrap();
    let updated = doc
        .set_comment(comment_path("items[0].name:inline"), "note")
        .unwrap();

    assert_eq!(
        updated
            .get_comment(comment_path("items[0].name:inline"))
            .unwrap(),
        Some("note".to_string())
    );
    // 値は変わらない
    assert_eq!(
        updated.get(object_path("items[0].name")).unwrap(),
        hjson_edit::HjsonValue::String("a".to_string())
    );
}
