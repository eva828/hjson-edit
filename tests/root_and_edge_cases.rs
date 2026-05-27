use hjson_edit::{
    ast::{DelimiterStyle, Trivia},
    high_level::{
        ops::object::{AppendMode, HjsonDocumentObjectEdit},
        path::HjsonObjectPath,
        HjsonDocument,
    },
};
use pretty_assertions::assert_eq;

#[test]
fn braceless_root_object_can_be_edited() {
    let input = "name: app\nport: 8080\n";
    let expected = "name: app\nport: 9090\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("port").unwrap(), 9090)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn braceless_root_first_member_owns_indent_after_header_trivia() {
    let input = "// This is a header comment\r\n\r\n// {\r\n    a: aaa,\r\n    b: bbb\r\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let ast = doc.ast().unwrap();

    assert!(matches!(
        ast.leading_trivia.last(),
        Some(Trivia::Newline { .. })
    ));
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

#[test]
fn braceless_root_object_uses_braceless_delimiter_style() {
    let input = "name: app\nport: 8080\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let ast = doc.ast().unwrap();
    let object = ast.object().unwrap();

    assert_eq!(object.delimiter_style, DelimiterStyle::Braceless);
}

#[test]
fn set_append_before_footer_inserts_member_above_braceless_footer_comment() {
    let input = "name: app\n# footer\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .append_mode(AppendMode::BeforeFooter);
    let updated = doc
        .set(HjsonObjectPath::parse("port").unwrap(), 8080)
        .unwrap();
    let output = updated.to_string();

    let port_pos = output.find("port: 8080").unwrap();
    let footer_pos = output.find("# footer").unwrap();
    assert!(port_pos < footer_pos, "output was: {output:?}");
}

#[test]
fn set_append_after_comments_inserts_member_below_braceless_footer_comment() {
    let input = "name: app\n# footer\n";

    let doc = HjsonDocument::parse(input)
        .unwrap()
        .append_mode(AppendMode::AfterComments);
    let updated = doc
        .set(HjsonObjectPath::parse("port").unwrap(), 8080)
        .unwrap();
    let output = updated.to_string();

    let port_pos = output.find("port: 8080").unwrap();
    let footer_pos = output.find("# footer").unwrap();
    assert!(port_pos > footer_pos, "output was: {output:?}");
}
