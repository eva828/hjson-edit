use std::convert::TryFrom;

use hjson_edit::{
    ast::CommentKind,
    high_level::{
        ops::comment::HjsonDocumentCommentEdit, path::HjsonCommentPath, HjsonDocument, Prefer,
    },
};

// ------------------------------------------------------------------
// Tests for comment operations on deeply nested paths.
// ------------------------------------------------------------------

#[test]
fn set_inline_comment_on_deep_path() {
    let input = "{\n  a: {\n    b: {\n      c: value\n    }\n  }\n}\n";
    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("a.b.c:inline").unwrap(), "note")
        .unwrap();
    assert!(updated.to_string().contains("// note"));
}

#[test]
fn get_inline_comment_on_deep_path_returns_comment() {
    let input = "{\n  a: {\n    b: {\n      c: \"value\" // note\n    }\n  }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let comment = doc
        .get_comment(HjsonCommentPath::try_from("a.b.c:inline").unwrap())
        .unwrap();
    assert_eq!(comment, Some("note".to_string()));
}

#[test]
fn remove_inline_comment_on_deep_path_removes_comment() {
    let input = "{\n  a: {\n    b: {\n      c: \"value\" // note\n    }\n  }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let removed = doc
        .remove_comment(HjsonCommentPath::try_from("a.b.c:inline").unwrap())
        .unwrap();
    assert!(!removed.to_string().contains("// note"));
}

#[test]
fn set_before_comment_on_deep_path() {
    let input = "{\n  a: {\n    b: {\n      c: value\n    }\n  }\n}\n";
    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("a.b.c:before").unwrap(), "note")
        .unwrap();
    assert!(updated.to_string().contains("// note"));
}
