//! 新規要素の字下げ（既存の要素と桁を揃えること）。

use hjson_edit::{
    HjsonDocument, HjsonDocumentEdit, HjsonDocumentMerge, HjsonObjectPath, HjsonValue,
};

fn path(path: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(path).unwrap()
}

fn set(input: &str, target: &str, value: impl Into<HjsonValue>) -> String {
    HjsonDocument::parse(input)
        .unwrap()
        .set(path(target), value)
        .unwrap()
        .to_string()
}

#[test]
fn new_member_uses_the_existing_indent() {
    assert_eq!(set("{\n  a: 1\n}\n", "b", 2), "{\n  a: 1\n  b: 2\n}\n");
    assert_eq!(
        set("{\n    a: 1\n}\n", "b", 2),
        "{\n    a: 1\n    b: 2\n}\n"
    );
    assert_eq!(set("{\n\ta: 1\n}\n", "b", 2), "{\n\ta: 1\n\tb: 2\n}\n");
}

#[test]
fn new_member_of_a_nested_container_uses_its_indent() {
    assert_eq!(
        set("{\n  o: {\n    a: 1\n  }\n}\n", "o.b", 2),
        "{\n  o: {\n    a: 1\n    b: 2\n  }\n}\n"
    );
}

#[test]
fn new_array_element_uses_the_existing_indent() {
    assert_eq!(
        set("{\n  a: [\n    1\n  ]\n}\n", "a[-]", 2),
        "{\n  a: [\n    1\n    2\n  ]\n}\n"
    );
    assert_eq!(set("[\n  1\n  3\n]\n", "[+1]", 2), "[\n  1\n  2\n  3\n]\n");
}

#[test]
fn new_member_of_an_empty_container_is_one_level_deeper() {
    assert_eq!(set("{\n}\n", "a", 1), "{\n  a: 1\n}\n");
    assert_eq!(
        set("{\n  o: {\n  }\n}\n", "o.b", 1),
        "{\n  o: {\n    b: 1\n  }\n}\n"
    );
    assert_eq!(set("[\n]\n", "[-]", 1), "[\n  1\n]\n");
}

#[test]
fn empty_container_keeps_the_closing_indent() {
    assert_eq!(
        set("{\n  o: {\n    }\n}\n", "o.b", 1),
        "{\n  o: {\n    b: 1\n    }\n}\n"
    );
}

#[test]
fn single_line_containers_stay_on_one_line() {
    assert_eq!(set("{a: 1}", "b", 2), "{a: 1, b: 2}");
    assert_eq!(set("[1, 3]", "[+1]", 2), "[1, 2, 3]");
    assert_eq!(set("[1,3]", "[+1]", 2), "[1,2,3]");
}

#[test]
fn crlf_documents_use_their_own_newline() {
    assert_eq!(
        set("{\r\n  a: 1\r\n}\r\n", "b", 2),
        "{\r\n  a: 1\r\n  b: 2\r\n}\r\n"
    );
}

#[test]
fn braceless_root_members_start_at_the_line_start() {
    assert_eq!(set("a: 1\n", "b", 2), "a: 1\nb: 2\n");
}

#[test]
fn merged_members_use_the_host_indent() {
    let host = HjsonDocument::parse("{\n  o: {\n    x: 1\n  }\n}\n").unwrap();
    let source = HjsonDocument::parse("{\n  // lead\n  // second\n  b: 2\n}\n").unwrap();

    let merged = host.merge_from(&path("o"), &source).unwrap();

    assert_eq!(
        merged.to_string(),
        "{\n  o: {\n    x: 1\n    // lead\n    // second\n    b: 2\n  }\n}\n"
    );
}

#[test]
fn inserted_element_keeps_the_container_separator_style() {
    assert_eq!(
        set("{\n  a: 1\n  c: 3\n}\n", "a", 0),
        "{\n  a: 0\n  c: 3\n}\n"
    );
    assert_eq!(
        set("{\n  a: 1\n  // footer\n}\n", "b", 2),
        "{\n  a: 1\n  b: 2\n  // footer\n}\n"
    );
}
