use std::convert::TryFrom;

use hjson_edit::{
    ast::Trivia,
    high_level::{
        ops::{
            comment::{CommentStyle, HjsonDocumentCommentEdit},
            object::{HjsonDocumentObjectEdit, StringStyle},
        },
        path::HjsonCommentPath,
        HjsonDocument,
    },
};
use pretty_assertions::assert_eq;

#[test]
fn set_push_preserves_crlf() {
    let input = "{\n  items: [\r\n    1\r\n    2\r\n  ]\n}\n";
    let expected = "{\n  items: [\r\n    1\r\n    2\r\n  3]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("items[-]", 3).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_updates_existing_array_item() {
    let input = "{\n  items: [1, 2, 3]\n}\n";
    let expected = "{\n  items: [1, 4, 3]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("items[1]", 4).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_updates_existing_object_value() {
    let input = "{\n  name: app\n}\n";
    let expected = "{\n  name: api\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("name", "api").unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_insert_at_appends_when_index_is_past_end() {
    let input = "{\n  items: [1, 2]\n}\n";
    let expected = "{\n  items: [1, 2, 3]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("items[+10]", 3).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_insert_at_inserts_in_the_middle() {
    let input = "{\n  items: [1, 3]\n}\n";
    let expected = "{\n  items: [1,2 3]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("items[+1]", 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_adds_nested_object_member() {
    let input = "{\n  server: { host: localhost }\n}\n";
    let expected = "{\n  server: { host: localhost , port: 8080}\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("server.port", 8080).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn remove_object_and_array_members() {
    let object_input = "{\n  server: { host: localhost, port: 8080 }\n}\n";
    let object_expected = "{\n  server: { host: localhost }\n}\n";
    let object_doc = HjsonDocument::parse(object_input).unwrap();
    let object_updated = object_doc.remove("server.port").unwrap();
    assert_eq!(object_updated.to_string(), object_expected);

    let array_input = "{\n  items: [1, 2, 3]\n}\n";
    let array_expected = "{\n  items: [1, 2]\n}\n";
    let array_doc = HjsonDocument::parse(array_input).unwrap();
    let array_updated = array_doc.remove("items[-]").unwrap();
    assert_eq!(array_updated.to_string(), array_expected);
}

#[test]
fn set_respects_explicit_string_styles() {
    let input = "{\n  name: \"app\"\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.string_style(StringStyle::Preserve).set("name", "api").unwrap();
    assert_eq!(updated.to_string(), "{\n  name: \"api\"\n}\n");

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.string_style(StringStyle::DoubleQuoted).set("name", "api").unwrap();
    assert_eq!(updated.to_string(), "{\n  name: \"api\"\n}\n");

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.string_style(StringStyle::Block).set("name", "api").unwrap();
    assert_eq!(updated.to_string(), "{\n  name: '''api'''\n}\n");

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.string_style(StringStyle::Unquoted).set("name", "api").unwrap();
    assert_eq!(updated.to_string(), "{\n  name: api\n}\n");
}

#[test]
fn header_comments_use_requested_style() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input).unwrap().comment_style(CommentStyle::Line);
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":header").unwrap(), "top")
        .unwrap();
    assert_eq!(updated.to_string(), "// top\n{\n  name: app\n}\n");

    let doc = HjsonDocument::parse(input).unwrap().comment_style(CommentStyle::Hash);
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":header").unwrap(), "top")
        .unwrap();
    assert_eq!(updated.to_string(), "# top\n{\n  name: app\n}\n");

    let doc = HjsonDocument::parse(input).unwrap().comment_style(CommentStyle::Block);
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":header").unwrap(), "top")
        .unwrap();
    assert_eq!(updated.to_string(), "/* top */\n{\n  name: app\n}\n");
}

#[test]
fn header_comment_can_be_removed() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input).unwrap().comment_style(CommentStyle::Line);
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
fn before_comment_is_inserted_before_the_member() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input).unwrap().comment_style(CommentStyle::Line);
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("name:before").unwrap(), "note")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  // note\n  name: app\n}\n");
}

#[test]
fn inline_comment_is_rendered_after_the_value() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input).unwrap().comment_style(CommentStyle::Line);
    let updated = doc
        .set_comment(HjsonCommentPath::try_from("name:inline").unwrap(), "note")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  name: app // note\n}\n");
}

#[test]
fn footer_comment_is_appended_after_the_document() {
    let input = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input).unwrap().comment_style(CommentStyle::Hash);
    let updated = doc
        .set_comment(HjsonCommentPath::try_from(":footer").unwrap(), "done")
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  name: app\n}\n# done\n");
}

#[test]
fn root_object_without_braces_can_be_edited() {
    let input = "name: app\nport: 8080\n";
    let expected = "name: app\nport: 9090\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("port", 9090).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_inserts_into_empty_object_and_array() {
    let obj_input = "{\n}\n";
    let obj_expected = "{\n  name: app\n}\n";
    let obj_doc = HjsonDocument::parse(obj_input).unwrap();
    let obj_updated = obj_doc.set("name", "app").unwrap();
    assert_eq!(obj_updated.to_string(), obj_expected);

    let arr_input = "[\n]\n";
    let arr_expected = "[\n  1\n]\n";
    let arr_doc = HjsonDocument::parse(arr_input).unwrap();
    let arr_updated = arr_doc.set("[-]", 1).unwrap();
    assert_eq!(arr_updated.to_string(), arr_expected);
}

#[test]
fn set_creates_missing_parents() {
    let input = "{\n  root: {\n  }\n}\n";
    let expected = "{\n  root: {\n    new: {\n      nested: value\n    }\n  }\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("root.new.nested", "value").unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_existing_key_preserves_comments() {
    let input = "{\n  // keep this note\n  name: app\n}\n";
    let expected = "{\n  // keep this note\n  name: api\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set("name", "api").unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn remove_last_item_drops_previous_comma() {
    let input = "{\n  keep: 1,\n  remove: 2\n}\n";
    let expected = "{\n  keep: 1\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.remove("remove").unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn braceless_root_first_member_owns_indent_after_header_trivia() {
    let input = "// This is a header comment\r\n\r\n// {\r\n    a: aaa,\r\n    b: bbb\r\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let ast = doc.ast().unwrap();

    assert!(matches!(ast.leading_trivia.last(), Some(Trivia::Newline { .. })));
    assert!(!matches!(
        ast.leading_trivia.last(),
        Some(Trivia::Whitespace(_))
    ));

    let object = ast.object().unwrap();
    let members = object.members();
    assert!(matches!(
        members.first().and_then(|member| member.leading_trivia.first()),
        Some(Trivia::Whitespace(indent)) if indent == "    "
    ));
}

#[test]
fn braceless_root_single_line_indent_is_not_document_leading_trivia() {
    let input = "    a: 1\n  b: 2\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let ast = doc.ast().unwrap();
    assert!(ast.leading_trivia.is_empty());

    let object = ast.object().unwrap();
    let members = object.members();
    assert!(matches!(
        members.first().and_then(|member| member.leading_trivia.first()),
        Some(Trivia::Whitespace(indent)) if indent == "    "
    ));
}
