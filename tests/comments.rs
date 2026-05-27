use std::convert::TryFrom;

use hjson_edit::{
    ast::CommentKind,
    high_level::{
        ops::comment::HjsonDocumentCommentEdit,
        path::HjsonCommentPath,
        Prefer, HjsonDocument,
    },
};
use pretty_assertions::assert_eq;

#[test]
fn set_header_comment_uses_explicit_line_style() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":header").unwrap(), "top")
        .unwrap();
    assert_eq!(updated.to_string(), "// top\n{\n  name: app\n}\n");
}

#[test]
fn set_header_comment_uses_explicit_hash_style() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Hash));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":header").unwrap(), "top")
        .unwrap();
    assert_eq!(updated.to_string(), "# top\n{\n  name: app\n}\n");
}

#[test]
fn set_header_comment_uses_explicit_block_style() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Block));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":header").unwrap(), "top")
        .unwrap();
    assert_eq!(updated.to_string(), "/* top */\n{\n  name: app\n}\n");
}

#[test]
fn set_then_remove_header_comment_roundtrips() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":header").unwrap(), "top")
        .unwrap();

    assert_eq!(
        updated
            .get_comment(HjsonCommentPath::try_from(":header").unwrap())
            .unwrap(),
        Some("top".to_string())
    );

    let removed = updated
        .remove_comment(HjsonCommentPath::try_from(":header").unwrap())
        .unwrap();
    assert_eq!(removed.to_string(), input);
}

#[test]
fn set_before_comment_inserts_above_member() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("name:before").unwrap(), "note")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  // note\n  name: app\n}\n");
}

#[test]
fn set_inline_comment_appends_after_value() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("name:inline").unwrap(), "note")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  name: app // note\n}\n");
}

#[test]
fn set_footer_comment_appends_after_document() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Hash));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":footer").unwrap(), "done")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  name: app\n}\n# done\n");
}

#[test]
fn set_then_remove_footer_comment_roundtrips() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Hash));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":footer").unwrap(), "done")
        .unwrap();

    assert_eq!(
        updated
            .get_comment(HjsonCommentPath::try_from(":footer").unwrap())
            .unwrap(),
        Some("done".to_string())
    );

    let removed = updated
        .remove_comment(HjsonCommentPath::try_from(":footer").unwrap())
        .unwrap();
    assert_eq!(removed.to_string(), input);
}

#[test]
fn remove_header_comment_preserves_leading_blank_lines() {
    let input = "\n\n# existing\n{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let removed = doc
        .remove_comment(HjsonCommentPath::try_from(":header").unwrap())
        .unwrap();
    assert_eq!(removed.to_string(), "\n\n{\n  name: app\n}\n");
}

#[test]
fn remove_footer_comment_preserves_trailing_blank_lines() {
    let input = "{\n  name: app\n}\n\n# existing\n\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let removed = doc
        .remove_comment(HjsonCommentPath::try_from(":footer").unwrap())
        .unwrap();
    assert_eq!(removed.to_string(), "{\n  name: app\n}\n\n\n");
}

#[test]
fn set_header_comment_preserves_existing_blank_lines() {
    let input = "\n\n{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":header").unwrap(), "top")
        .unwrap();
    assert_eq!(updated.to_string(), "\n\n// top\n{\n  name: app\n}\n");
}

#[test]
fn set_footer_comment_preserves_existing_blank_lines() {
    let input = "{\n  name: app\n}\n\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Hash));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":footer").unwrap(), "done")
        .unwrap();
    assert_eq!(updated.to_string(), "{\n  name: app\n}\n\n# done\n");
}
