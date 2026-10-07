use std::convert::TryFrom;

use hjson_edit::{
    ast::CommentKind,
    high_level::{
        ops::comment::HjsonDocumentCommentEdit, path::HjsonCommentPath, HjsonDocument, Prefer,
    },
};
use pretty_assertions::assert_eq;

#[test]
fn append_comment_to_existing_header_appends_below() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":header").unwrap(), "first")
        .unwrap();
    let appended = updated
        .append_comment(HjsonCommentPath::try_from(":header").unwrap(), "second")
        .unwrap();

    assert_eq!(
        appended.to_string(),
        "// first\n// second\n{\n  name: app\n}\n"
    );
}

#[test]
fn append_comment_to_existing_inline_appends_inline() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("name:inline").unwrap(), "first")
        .unwrap();
    let appended = updated
        .append_comment(HjsonCommentPath::try_from("name:inline").unwrap(), "second")
        .unwrap();

    // Multi-line text forces Block style regardless of original inline style.
    assert_eq!(appended.to_string(), "{\n  name: app/* first\nsecond */}\n");
}

#[test]
fn append_comment_to_member_without_existing_comment_inserts_new() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .append_comment(HjsonCommentPath::try_from("name:before").unwrap(), "note")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  // note\nname: app\n}\n");
}

#[test]
fn append_comment_to_footer_appends_below_existing() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Hash));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":footer").unwrap(), "first")
        .unwrap();
    let appended = updated
        .append_comment(HjsonCommentPath::try_from(":footer").unwrap(), "second")
        .unwrap();

    assert_eq!(appended.to_string(), "{\n  name: app\n}\n# first\n# second");
}
