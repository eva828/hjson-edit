//! 1 行の文書（要素が前の内容と同じ行にある）でのコメント操作。
//!
//! `:before` はコメントを独立した行に置き、`:inline` は同じ行に置く。
//! 直前の要素と同じ行にあるコメントは、その要素の `:inline` としてだけ数える。

use hjson_edit::high_level::ops::comment::HjsonDocumentCommentEdit;
use hjson_edit::high_level::ops::object::HjsonDocumentEdit;
use hjson_edit::high_level::path::{HjsonCommentPath, HjsonObjectPath};
use hjson_edit::{HjsonDocument, HjsonEditError, HjsonValue};

fn comment_path(raw: &str) -> HjsonCommentPath {
    HjsonCommentPath::try_from(raw).unwrap()
}

fn doc(src: &str) -> HjsonDocument {
    HjsonDocument::parse(src).unwrap()
}

#[test]
fn a_comment_on_the_same_line_as_the_previous_element_is_inline_only() {
    let parsed = doc("{a: 1, /* c */ b: 2}\n");

    assert_eq!(
        parsed
            .get_comment(comment_path("a:inline"))
            .unwrap()
            .as_deref(),
        Some("c")
    );
    assert_eq!(parsed.get_comment(comment_path("b:before")).unwrap(), None);
}

#[test]
fn inline_on_a_single_line_object_is_the_inline_of_its_own_element() {
    let updated = doc("{a: 1, b: 2}\n")
        .set_comment(comment_path("a:inline"), "note")
        .unwrap();

    assert_eq!(
        updated
            .get_comment(comment_path("a:inline"))
            .unwrap()
            .as_deref(),
        Some("note")
    );
    // 同じ行でも、カンマの後ろのコメントは前の要素のもの
    assert_eq!(updated.get_comment(comment_path("b:before")).unwrap(), None);
}

#[test]
fn inline_on_a_single_line_array_works_for_every_element() {
    for slot in ["[0]:inline", "[1]:inline"] {
        let updated = doc("[1, 2]\n")
            .set_comment(comment_path(slot), "note")
            .unwrap();

        assert_eq!(
            updated.get_comment(comment_path(slot)).unwrap().as_deref(),
            Some("note"),
            "{slot} -> {updated}"
        );
    }
}

#[test]
fn before_moves_a_comment_of_a_single_line_object_to_its_own_line() {
    for (src, slot) in [
        ("{a: 1, b: 2}\n", "a:before"),
        ("{a: 1, b: 2}\n", "b:before"),
        ("{a: 1\n  b: 2\n}\n", "a:before"),
        ("[1, 2]\n", "[0]:before"),
        ("[1, 2]\n", "[1]:before"),
        ("x: {a: 1}\n", "x.a:before"),
    ] {
        let updated = doc(src)
            .set_comment(comment_path(slot), "note")
            .unwrap_or_else(|error| panic!("{src:?} {slot}: {error}"));

        assert_eq!(
            updated.get_comment(comment_path(slot)).unwrap().as_deref(),
            Some("note"),
            "{src:?} {slot} -> {updated}"
        );
        assert!(
            updated.to_string().contains("\n  /* note */\n"),
            "{src:?} {slot} -> {updated}"
        );
    }
}

#[test]
fn before_keeps_the_values_of_a_single_line_object() {
    let updated = doc("{a: 1, b: 2}\n")
        .set_comment(comment_path("b:before"), "note")
        .unwrap();

    assert_eq!(
        updated.get(HjsonObjectPath::parse("a").unwrap()).unwrap(),
        HjsonValue::Integer(1)
    );
    assert_eq!(
        updated.get(HjsonObjectPath::parse("b").unwrap()).unwrap(),
        HjsonValue::Integer(2)
    );
}

#[test]
fn before_on_the_first_member_of_a_braceless_document_suggests_the_header() {
    let error = doc("a: 1\nb: 2\n")
        .set_comment(comment_path("a:before"), "note")
        .unwrap_err();

    assert!(
        matches!(error, HjsonEditError::InvalidTarget { .. }),
        "{error:?}"
    );
    assert!(error.to_string().contains("use :header"), "{error}");
}

#[test]
fn append_adds_a_line_and_remove_clears_the_slot() {
    let updated = doc("{a: 1, b: 2}\n")
        .append_comment(comment_path("a:before"), "first")
        .unwrap()
        .append_comment(comment_path("a:before"), "second")
        .unwrap();
    assert_eq!(
        updated
            .get_comment(comment_path("a:before"))
            .unwrap()
            .as_deref(),
        Some("first\nsecond")
    );

    let removed = updated.remove_comment(comment_path("a:before")).unwrap();
    assert_eq!(removed.get_comment(comment_path("a:before")).unwrap(), None);
    assert!(removed.all_comments().is_empty(), "{removed}");
}

#[test]
fn setting_the_same_slot_twice_replaces_the_comment() {
    let updated = doc("{a: 1, b: 2}\n")
        .set_comment(comment_path("a:before"), "first")
        .unwrap()
        .set_comment(comment_path("a:before"), "second")
        .unwrap();

    assert_eq!(updated.all_comments(), vec!["second".to_string()]);
}

#[test]
fn formatting_keeps_the_comment_of_a_single_line_document() {
    let updated = doc("{a: 1, b: 2}\n")
        .set_comment(comment_path("b:before"), "note")
        .unwrap();
    let config = updated.detected_format_config();
    let formatted = updated
        .format_container(&HjsonObjectPath::parse("").unwrap(), &config)
        .unwrap();
    let reparsed = HjsonDocument::parse(&formatted.to_string()).unwrap();

    assert_eq!(
        reparsed
            .get_comment(comment_path("b:before"))
            .unwrap()
            .as_deref(),
        Some("note"),
        "{formatted}"
    );
}

#[test]
fn a_comment_on_the_opening_delimiter_line_is_the_first_element_before() {
    for (src, slot, expected) in [
        ("{/* c */\na: 1}\n", "a:before", "c"),
        ("{#c\n a: 1}\n", "a:before", "c"),
        ("{ /* c */\n  a: 1}\n", "a:before", "c"),
        ("[/* c */\n1]\n", "[0]:before", "c"),
        ("x: {/* c */\na: 1}\n", "x.a:before", "c"),
    ] {
        let parsed = doc(src);

        assert_eq!(
            parsed.get_comment(comment_path(slot)).unwrap().as_deref(),
            Some(expected),
            "{src:?} {slot}"
        );
    }
}

#[test]
fn setting_before_replaces_a_comment_on_the_opening_delimiter_line() {
    for (src, slot, expected) in [
        ("{/* c */\na: 1}\n", "a:before", "{/* note */\na: 1}\n"),
        ("{#c\n a: 1}\n", "a:before", "{# note\n a: 1}\n"),
        ("[/* c */\n1]\n", "[0]:before", "[/* note */\n1]\n"),
    ] {
        let updated = doc(src).set_comment(comment_path(slot), "note").unwrap();

        assert_eq!(updated.to_string(), expected, "{src:?}");
        assert_eq!(updated.all_comments(), vec!["note".to_string()], "{src:?}");
    }
}

#[test]
fn removing_before_clears_a_comment_on_the_opening_delimiter_line() {
    let updated = doc("{/* c */\na: 1}\n")
        .remove_comment(comment_path("a:before"))
        .unwrap();

    assert_eq!(updated.to_string(), "{a: 1}\n");
    assert!(updated.all_comments().is_empty());
}

#[test]
fn a_new_before_comment_still_gets_its_own_line() {
    let updated = doc("{a: 1, b: 2}\n")
        .set_comment(comment_path("a:before"), "note")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  /* note */\n  a: 1, b: 2}\n");
}

#[test]
fn a_braceless_document_keeps_its_header_separate() {
    let parsed = doc("/* header */\na: 1\n");

    assert_eq!(
        parsed
            .get_comment(comment_path(":header"))
            .unwrap()
            .as_deref(),
        Some("header")
    );
    assert_eq!(parsed.get_comment(comment_path("a:before")).unwrap(), None);
}
