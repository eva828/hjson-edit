//! 公開前レビューで見つかった欠陥の回帰テスト。
//!
//! - 配列要素のコメント操作（親メンバーを書き換えない）
//! - `merge_from` がコメントを保つ / ブレースレスのマージで値を壊さない
//! - `[+]` が末尾を指す
//! - 末尾コメントの手前へ追加する
//! - 重複キーのある文書でも値が一致する

use hjson_edit::{
    high_level::{
        ops::{
            comment::HjsonDocumentCommentEdit, merge::HjsonDocumentMerge, object::HjsonDocumentEdit,
        },
        path::{HjsonCommentPath, HjsonObjectPath},
    },
    HjsonDocument, HjsonValue,
};

fn path(raw: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(raw.to_string()).expect("valid path")
}

fn comment_path(raw: &str) -> HjsonCommentPath {
    HjsonCommentPath::try_from(raw).expect("valid comment path")
}

fn value(doc: &HjsonDocument, raw: &str) -> HjsonValue {
    doc.get(path(raw)).expect("value")
}

/// 編集結果が再パースでき、値が期待どおりであることを確認する。
fn assert_roundtrip(doc: &HjsonDocument) {
    let text = doc.to_string();
    HjsonDocument::parse(&text)
        .unwrap_or_else(|error| panic!("edited document does not parse: {error}\n{text}"));
}

// ------------------------------------------------------------------
// ブレースレスのマージ（値の破壊）
// ------------------------------------------------------------------

#[test]
fn merge_into_braceless_root_keeps_values() {
    let host = HjsonDocument::parse("a: 1\n").unwrap();
    let source = HjsonDocument::parse("{\n  b: 2\n  c: 3\n}").unwrap();

    let merged = host.merge_from(&path(""), &source).unwrap();
    assert_roundtrip(&merged);
    assert_eq!(value(&merged, "a"), HjsonValue::Integer(1));
    assert_eq!(value(&merged, "b"), HjsonValue::Integer(2));
    assert_eq!(value(&merged, "c"), HjsonValue::Integer(3));
}

#[test]
fn merge_into_braceless_root_without_trailing_newline() {
    let host = HjsonDocument::parse("a: 1").unwrap();
    let source = HjsonDocument::parse("{b: 2}").unwrap();

    let merged = host.merge_from(&path(""), &source).unwrap();
    assert_roundtrip(&merged);
    assert_eq!(value(&merged, "a"), HjsonValue::Integer(1));
    assert_eq!(value(&merged, "b"), HjsonValue::Integer(2));
}

#[test]
fn merge_at_missing_path_in_braceless_document() {
    let host = HjsonDocument::parse("a: 1\n").unwrap();
    let source = HjsonDocument::parse("{b: 2}").unwrap();

    let merged = host.merge_from(&path("x"), &source).unwrap();
    assert_roundtrip(&merged);
    assert_eq!(value(&merged, "x.b"), HjsonValue::Integer(2));
    assert_eq!(value(&merged, "a"), HjsonValue::Integer(1));
}

// ------------------------------------------------------------------
// マージとコメント
// ------------------------------------------------------------------

#[test]
fn merge_copies_source_comments() {
    let host = HjsonDocument::parse("{\n  keep: 1\n}\n").unwrap();
    let source = HjsonDocument::parse("{\n  // lead\n  a: 1 // inline\n}").unwrap();

    let merged = host.merge_from(&path(""), &source).unwrap();
    assert_roundtrip(&merged);
    assert_eq!(value(&merged, "a"), HjsonValue::Integer(1));
    assert_eq!(value(&merged, "keep"), HjsonValue::Integer(1));
    assert_eq!(
        merged.to_string(),
        "{\n  keep: 1\n  // lead\n  a: 1 // inline\n}\n"
    );
}

#[test]
fn merge_keeps_host_comments_on_replaced_member() {
    let host = HjsonDocument::parse("{\n  a: 1 // host note\n}\n").unwrap();
    let source = HjsonDocument::parse("{\n  a: 2 // source note\n}").unwrap();

    let merged = host.merge_from(&path(""), &source).unwrap();
    assert_roundtrip(&merged);
    assert_eq!(value(&merged, "a"), HjsonValue::Integer(2));
    // 置き換えではホスト側のキーとコメントを保つ
    assert!(merged.to_string().contains("host note"));
}

// ------------------------------------------------------------------
// パス `[+]`
// ------------------------------------------------------------------

#[test]
fn push_segment_appends_to_array() {
    let doc = HjsonDocument::parse("{\n  items: [\n    1\n  ]\n}\n").unwrap();

    let updated = doc.clone().set(path("items[+]"), 2).unwrap();
    assert_roundtrip(&updated);
    assert_eq!(
        value(&updated, "items"),
        HjsonValue::Array(vec![HjsonValue::Integer(1), HjsonValue::Integer(2)])
    );

    // 読み取りは末尾要素
    assert_eq!(doc.get(path("items[+]")).unwrap(), HjsonValue::Integer(1));
}

#[test]
fn push_segment_removes_last_element() {
    let doc = HjsonDocument::parse("[\n  1\n  2\n]\n").unwrap();
    let updated = doc.remove(path("[+]")).unwrap();

    assert_eq!(updated.to_string(), "[\n  1\n]\n");
}

// ------------------------------------------------------------------
// 末尾コメント（footer）の手前へ追加
// ------------------------------------------------------------------

#[test]
fn append_goes_before_footer_comment() {
    let doc = HjsonDocument::parse("{\n  a: 1\n  // footer\n}\n").unwrap();
    let updated = doc.set(path("b"), 2).unwrap();

    assert_eq!(updated.to_string(), "{\n  a: 1\n  b: 2\n  // footer\n}\n");
}

#[test]
fn append_to_array_goes_before_footer_comment() {
    let doc = HjsonDocument::parse("[\n  1\n  // footer\n]\n").unwrap();
    let updated = doc.set(path("[+]"), 2).unwrap();

    assert_eq!(updated.to_string(), "[\n  1\n  2\n  // footer\n]\n");
}

// ------------------------------------------------------------------
// 重複キー
// ------------------------------------------------------------------

#[test]
fn set_on_duplicate_key_updates_every_member() {
    let doc = HjsonDocument::parse("{\n  a: 1\n  a: 2\n}\n").unwrap();
    let updated = doc.set(path("a"), 3).unwrap();

    assert_roundtrip(&updated);
    // 値の層では後勝ちなので、両方を更新して同じ値にする
    assert_eq!(value(&updated, "a"), HjsonValue::Integer(3));
    assert_eq!(updated.to_string(), "{\n  a: 3\n  a: 3\n}\n");
}

#[test]
fn remove_on_duplicate_key_removes_every_member() {
    let doc = HjsonDocument::parse("{\n  a: 1\n  a: 2\n  b: 3\n}\n").unwrap();
    let updated = doc.remove(path("a")).unwrap();

    assert_roundtrip(&updated);
    assert_eq!(updated.to_string(), "{\n  b: 3\n}\n");
}

// ------------------------------------------------------------------
// コメント編集は他のコメントを壊さない
// ------------------------------------------------------------------

#[test]
fn comment_edits_keep_other_comments() {
    let input = "{\n  // first\n  a: 1 // inline\n  b: [\n    1 // item\n  ]\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();

    let updated = doc
        .clone()
        .set_comment(comment_path("b[0]:inline"), "changed")
        .unwrap();
    assert_roundtrip(&updated);
    assert!(updated.to_string().contains("// first"));
    assert!(updated.to_string().contains("a: 1 // inline"));
    // 既存のコメントが行コメントなので、行コメントのまま置き換える
    assert!(updated.to_string().contains("1 // changed"));

    let updated = doc
        .clone()
        .remove_comment(comment_path("a:inline"))
        .unwrap();
    assert_roundtrip(&updated);
    assert!(updated.to_string().contains("// first"));
    assert!(updated.to_string().contains("// item"));
}
