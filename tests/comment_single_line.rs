//! 1 行の文書（要素が前の内容と同じ行にある）でのコメント操作。
//!
//! `:before` はコメントを独立した行に置き、`:inline` は同じ行に置く。
//! 直前の要素と同じ行にあるコメントは、その要素の `:inline` としてだけ数える。

use hjson_edit::high_level::ops::comment::HjsonDocumentCommentEdit;
use hjson_edit::high_level::path::HjsonCommentPath;
use hjson_edit::HjsonDocument;

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
