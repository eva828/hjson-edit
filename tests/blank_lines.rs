//! 整形が空行を保つこと（翻訳データのレビューで差分を増やさない）。

use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::{HjsonDocument, HjsonDocumentEdit};

fn empty() -> HjsonObjectPath {
    HjsonObjectPath::from_segments(Vec::new())
}

fn format(input: &str) -> String {
    let document = HjsonDocument::parse(input).unwrap();
    document
        .format_container(&empty(), &document.current_format_config())
        .unwrap()
        .to_string()
}

#[test]
fn blank_lines_between_members_are_preserved() {
    let input = "{\n  a: 1\n\n  b: 2\n\n\n  c: 3\n}\n";

    assert_eq!(format(input), input);
}

#[test]
fn blank_lines_around_comments_are_preserved() {
    let input = "{\n  a: 1\n\n\n  // c\n\n  b: 2\n}\n";

    assert_eq!(format(input), input);
}

#[test]
fn a_blank_line_before_the_closing_brace_is_preserved() {
    let input = "{\n  a: 1\n\n}\n";

    assert_eq!(format(input), input);
}

#[test]
fn a_blank_line_after_the_opening_brace_is_preserved() {
    let input = "{\n\n  a: 1\n}\n";

    assert_eq!(format(input), input);
}

#[test]
fn blank_lines_in_arrays_are_preserved() {
    let input = "[\n  1\n\n  2\n\n]\n";

    assert_eq!(format(input), input);
}

#[test]
fn trailing_blank_lines_of_a_braceless_document_are_preserved() {
    for input in ["a: 1\n\n", "a: 1\n\n\n", "a: 1\n\nb: 2\n\n"] {
        assert_eq!(format(input), input);
    }
}

#[test]
fn formatting_with_blank_lines_is_idempotent() {
    let input = "{\n\n  a: 1\n\n  // c\n\n  b: [\n    1\n\n    2\n  ]\n\n}\n";
    let once = format(input);

    assert_eq!(format(&once), once);
}

#[test]
fn blank_lines_do_not_change_values() {
    let input = "{\n\n  a: 1\n\n  b: '''\n\n  x\n\n  '''\n\n}\n";
    let document = HjsonDocument::parse(input).unwrap();
    let formatted = document
        .format_container(&empty(), &document.current_format_config())
        .unwrap();

    for path in ["a", "b"] {
        let path = HjsonObjectPath::parse(path).unwrap();
        assert_eq!(
            document.get(path.clone()).unwrap(),
            formatted.get(path).unwrap()
        );
    }
}
