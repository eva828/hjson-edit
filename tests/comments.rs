use std::convert::TryFrom;

use hjson_edit::{
    ast::CommentKind,
    high_level::{
        ops::comment::HjsonDocumentCommentEdit, path::HjsonCommentPath, HjsonDocument, Prefer,
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

    // 追加したコメント行は要素の字下げを引き継ぐ
    assert_eq!(updated.to_string(), "{\n  // note\n  name: app\n}\n");
}

#[test]
fn set_inline_comment_appends_after_value() {
    // 引用符なし文字列は行末までを飲み込むため、引用符付きの値を対象にする
    let input = "{\n  name: \"app\"\n}\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Line));
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("name:inline").unwrap(), "note")
        .unwrap();

    // インラインコメントの前には空白を入れる
    assert_eq!(updated.to_string(), "{\n  name: \"app\" // note\n}\n");
}

#[test]
fn set_inline_comment_on_unquoted_string_is_rejected() {
    // 引用符なし文字列の直後にコメントを置くと値に吸収されるため、エラーにする
    let doc = HjsonDocument::parse("{\n  name: app\n}\n").unwrap();
    let error = doc
        .set_comment(HjsonCommentPath::try_from("name:inline").unwrap(), "note")
        .unwrap_err();
    assert!(
        matches!(error, hjson_edit::HjsonEditError::InvalidTarget { .. }),
        "unexpected error: {error:?}"
    );
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

    assert_eq!(updated.to_string(), "{\n  name: app\n}\n# done");
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
    assert_eq!(updated.to_string(), "{\n  name: app\n}\n\n# done");
}

#[test]
fn remove_missing_header_comment_is_noop() {
    let src = "{\n  foo: 1\n}";
    let doc = HjsonDocument::parse(src).unwrap();
    let new_doc = doc
        .remove_comment(HjsonCommentPath::try_from(":header").unwrap())
        .unwrap();
    assert_eq!(new_doc.to_string(), src);
}

#[test]
fn set_before_comment_overwrites_existing() {
    let src = "{\n  // old comment\n  foo: 1\n}";
    let doc = HjsonDocument::parse(src).unwrap();
    let new_doc = doc
        .set_comment(
            HjsonCommentPath::try_from("foo:before").unwrap(),
            "new comment",
        )
        .unwrap();
    // 置き換えても要素の字下げは残る
    let expected = "{\n  // new comment\n  foo: 1\n}";
    assert_eq!(new_doc.to_string(), expected);
}

#[test]
fn set_multiline_header_comment_with_line_style_splits_into_lines() {
    let src = "{\n  foo: 1\n}";
    let doc = HjsonDocument::parse(src).unwrap();
    let new_doc = doc
        .comment_style(Prefer::Explicit(CommentKind::Line))
        .set_comment(
            HjsonCommentPath::try_from(":header").unwrap(),
            "line1\nline2",
        )
        .unwrap();
    let expected = "// line1\n// line2\n{\n  foo: 1\n}";
    assert_eq!(new_doc.to_string(), expected);
}

#[test]
fn all_comments_lists_bodies_in_order() {
    let input = "// first\na: 1 // inline\n# hash\nb: 2 /* block */\n";

    let doc = HjsonDocument::parse(input).unwrap();

    assert_eq!(
        doc.all_comments(),
        vec![
            "first".to_string(),
            "inline".to_string(),
            "hash".to_string(),
            "block".to_string(),
        ]
    );
}

#[test]
fn inserted_comment_uses_a_hash_dominant_document() {
    let input = "# first\na: 1\nb: 2\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("b:before").unwrap(), "note")
        .unwrap();

    assert_eq!(updated.to_string(), "# first\na: 1\n# note\nb: 2\n");
}

#[test]
fn inserted_comment_falls_back_to_a_block_comment() {
    let input = "{\n  a: 1\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("a:before").unwrap(), "note")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  /* note */\n  a: 1\n}\n");
}

#[test]
fn a_block_comment_body_with_the_closing_marker_falls_back_to_a_line_comment() {
    // 既定（ブロック）スタイルでも、`*/` を含む本文は行コメントで書く
    let doc = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("a:before").unwrap(), "x */ y")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  // x */ y\n  a: 1\n}\n");
    assert_eq!(updated.all_comments(), vec!["x */ y".to_string()]);
}

#[test]
fn an_explicit_block_style_rejects_the_closing_marker() {
    let doc = HjsonDocument::parse("{\n  a: 1\n}\n")
        .unwrap()
        .comment_style(Prefer::Explicit(CommentKind::Block));
    let error = doc
        .set_comment(HjsonCommentPath::try_from("a:before").unwrap(), "x */ y")
        .unwrap_err();

    assert!(error.to_string().contains("cannot contain '*/'"), "{error}");
}

#[test]
fn a_header_comment_with_the_closing_marker_falls_back_to_a_line_comment() {
    let doc = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":header").unwrap(), "x */ y")
        .unwrap();

    assert_eq!(updated.to_string(), "// x */ y\n{\n  a: 1\n}\n");
}

#[test]
fn an_inline_comment_that_must_be_a_block_rejects_the_closing_marker() {
    // 行末でない位置は行コメントを置けないので、ブロックで書けない本文は拒否する
    let doc = HjsonDocument::parse("{a: 1}\n").unwrap();
    let error = doc
        .set_comment(HjsonCommentPath::try_from("a:inline").unwrap(), "x */ y")
        .unwrap_err();

    assert!(error.to_string().contains("cannot contain '*/'"), "{error}");
}

#[test]
fn an_inline_comment_at_the_end_of_a_line_uses_a_line_comment() {
    let doc = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("a:inline").unwrap(), "x */ y")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  a: 1 // x */ y\n}\n");
}
