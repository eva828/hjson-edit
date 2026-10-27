use hjson_edit::{
    ast::{DelimiterStyle, DocumentItem, ScalarValue, Trivia, ValueContent},
    high_level::HjsonDocument,
};

#[test]
fn nested_braced_object_uses_braced_delimiter_and_keeps_edge_trivia_separate() {
    let input = "k: {\r\n    l: lll\r\n      m: mm\r\n    }\r\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let ast = doc.ast().unwrap();
    let root_object = ast.object().unwrap();
    let k_member = root_object.member_by_key("k").unwrap();
    let nested_object = match k_member.value().unwrap().content().unwrap() {
        ValueContent::Object(object) => object,
        other => panic!("expected nested object, found {other:?}"),
    };

    assert_eq!(nested_object.delimiter_style, DelimiterStyle::Braced);
    assert!(!nested_object
        .leading_trivia
        .iter()
        .any(|trivia| matches!(trivia, Trivia::Whitespace(text) if text == "      ")));
    assert!(!nested_object
        .trailing_trivia
        .iter()
        .any(|trivia| matches!(trivia, Trivia::Whitespace(text) if text == "      ")));
}

#[test]
fn braced_array_reports_braced_delimiter_style() {
    let input = "[\n  1\n]\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let ast = doc.ast().unwrap();
    let array = ast.array().unwrap();

    assert_eq!(array.delimiter_style, DelimiterStyle::Braced);
}

#[test]
fn root_scalar_trivia_is_owned_by_document_not_value() {
    let input = "\r\n// header\r\n42\r\n// footer\r\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let ast = doc.ast().unwrap();

    assert!(matches!(ast.item(), Some(DocumentItem::Value(_))));
    assert!(ast
        .leading_trivia
        .iter()
        .any(|trivia| matches!(trivia, Trivia::Comment(_))));
    assert!(ast
        .trailing_trivia
        .iter()
        .any(|trivia| matches!(trivia, Trivia::Comment(_))));

    let value = ast.value().unwrap();
    match value.content().unwrap() {
        ValueContent::Scalar(ScalarValue::Integer { raw, .. }) => assert_eq!(raw, "42"),
        other => panic!("expected root scalar integer, found {other:?}"),
    }
}

#[test]
fn array_item_owns_item_trivia_and_exposes_value() {
    let input = "[\n  // first\n  1,\n  2\n]\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let ast = doc.ast().unwrap();
    let array = ast.array().unwrap();
    let items = array.items();

    assert_eq!(items.len(), 2);
    assert!(items[0]
        .leading_trivia
        .iter()
        .any(|trivia| { matches!(trivia, Trivia::Comment(comment) if comment.body == "first") }));

    match items[0].value().unwrap().content().unwrap() {
        ValueContent::Scalar(ScalarValue::Integer { raw, .. }) => assert_eq!(raw, "1"),
        other => panic!("expected first array item integer, found {other:?}"),
    }
}

#[test]
fn member_trivia_is_attributed_to_the_member() {
    let input = "{\n  a: 1 // inline\n  // lead\n  b: 2\n}\n";
    let document = HjsonDocument::parse(input).unwrap();
    let object = document.ast().unwrap().object().unwrap();

    let a = object.member_by_key("a").unwrap();
    assert!(
        a.trailing_trivia
            .iter()
            .any(|trivia| matches!(trivia, Trivia::Comment(comment) if comment.body == "inline")),
        "a: {:?}",
        a.trailing_trivia
    );

    let b = object.member_by_key("b").unwrap();
    assert!(
        b.leading_trivia
            .iter()
            .any(|trivia| matches!(trivia, Trivia::Comment(comment) if comment.body == "lead")),
        "b: {:?}",
        b.leading_trivia
    );
}

#[test]
fn leading_trivia_skips_the_previous_line_break() {
    let input = "{\n  a: 1\n\n  b: 2\n}\n";
    let document = HjsonDocument::parse(input).unwrap();
    let object = document.ast().unwrap().object().unwrap();

    let b = object.member_by_key("b").unwrap();
    let newlines = b
        .leading_trivia
        .iter()
        .filter(|trivia| matches!(trivia, Trivia::Newline { .. }))
        .count();

    assert_eq!(newlines, 1, "b: {:?}", b.leading_trivia);
}

#[test]
fn comma_separated_members_have_no_leading_trivia() {
    let input = "{a: 1, b: 2}\n";
    let document = HjsonDocument::parse(input).unwrap();
    let object = document.ast().unwrap().object().unwrap();

    // カンマの後ろの空白だけが先行トリビアになる（改行やコメントは入らない）
    assert_eq!(
        object.member_by_key("b").unwrap().leading_trivia,
        vec![Trivia::Whitespace(" ".to_string())]
    );
}

#[test]
fn a_comment_only_container_keeps_it_in_the_leading_trivia() {
    let input = "{\n  // only\n}\n";
    let document = HjsonDocument::parse(input).unwrap();
    let object = document.ast().unwrap().object().unwrap();

    assert!(object.members().is_empty());
    assert!(
        object
            .leading_trivia
            .iter()
            .any(|trivia| matches!(trivia, Trivia::Comment(comment) if comment.body == "only")),
        "leading: {:?}",
        object.leading_trivia
    );
}

#[test]
fn a_trailing_comment_of_the_last_member_stays_with_the_container() {
    let input = "{\n  a: 1\n  // last\n}\n";
    let document = HjsonDocument::parse(input).unwrap();
    let object = document.ast().unwrap().object().unwrap();

    assert!(
        object
            .trailing_trivia
            .iter()
            .any(|trivia| matches!(trivia, Trivia::Comment(comment) if comment.body == "last")),
        "trailing: {:?}",
        object.trailing_trivia
    );
}
