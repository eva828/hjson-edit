//! ブレースレス文書の整形。
//!
//! 末尾のコメントは本文（Object）のトリビアであり、`format_container` で失ってはならない。
//! また、本文の中のコンテナは文書全体の整形と同じ深さで整形する。

use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::HjsonDocument;

fn path(raw: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(raw).unwrap()
}

fn format_container(document: &HjsonDocument, raw: &str) -> String {
    let config = document.detected_format_config();
    document
        .format_container(&path(raw), &config)
        .unwrap()
        .to_string()
}

#[test]
fn format_container_keeps_the_trailing_comment() {
    let input = "a: 1\n// footer\n";
    let document = HjsonDocument::parse(input).unwrap();

    assert_eq!(format_container(&document, ""), input);
}

#[test]
fn format_container_keeps_trailing_blank_lines_and_comments() {
    let input = "a: 1\n\n// c\n\n";
    let document = HjsonDocument::parse(input).unwrap();

    assert_eq!(format_container(&document, ""), input);
}

#[test]
fn nested_container_in_a_braceless_document_is_formatted_at_the_document_depth() {
    let input = "name: app\nserver: {\n  host: localhost\n}\n";
    let document = HjsonDocument::parse(input).unwrap();

    assert_eq!(
        format_container(&document, "server"),
        format_container(&document, ""),
    );
}

#[test]
fn array_element_in_a_braceless_document_is_formatted_at_the_document_depth() {
    let input = "items: [\n  {a: 1}\n]\n";
    let document = HjsonDocument::parse(input).unwrap();

    assert_eq!(
        format_container(&document, "items[0]"),
        format_container(&document, ""),
    );
}
