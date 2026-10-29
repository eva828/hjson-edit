//! まとめて挿入する merge の挙動（重複キー・コメント・並び順）。

use hjson_edit::high_level::{
    ops::object::HjsonDocumentEdit, path::HjsonObjectPath, HjsonDocument,
};
use hjson_edit::{HjsonDocumentMerge, HjsonValue};

fn path(raw: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(raw).unwrap()
}

fn empty() -> HjsonObjectPath {
    HjsonObjectPath::from_segments(Vec::new())
}

#[test]
fn merging_updates_every_duplicate_key() {
    let host = HjsonDocument::parse("{\n  a: 1\n  a: 2\n  b: 3\n}\n").unwrap();
    let source = HjsonDocument::parse("{\n  a: 9\n}\n").unwrap();

    let merged = host.merge_from(&empty(), &source).unwrap();

    assert_eq!(merged.to_string(), "{\n  a: 9\n  a: 9\n  b: 3\n}\n");
    assert_eq!(merged.get(path("a")).unwrap(), HjsonValue::Integer(9));
}

#[test]
fn merging_many_new_members_keeps_order_and_footer() {
    let mut source_body = String::new();
    for index in 0..50 {
        source_body.push_str(&format!("  k{index}: {index}\n"));
    }
    let source_text = format!("{{\n{source_body}}}\n");

    let host = HjsonDocument::parse("{\n  keep: 1\n  // footer\n}\n").unwrap();
    let source = HjsonDocument::parse(&source_text).unwrap();
    let merged = host.merge_from(&empty(), &source).unwrap();

    let text = merged.to_string();
    let keep = text.find("keep: 1").unwrap();
    let mut previous = keep;
    for index in 0..50 {
        let position = text.find(&format!("k{index}: {index}")).unwrap();
        assert!(position > previous, "k{index} is out of order");
        previous = position;
    }
    assert!(previous < text.find("// footer").unwrap());
    assert!(HjsonDocument::parse(&text).is_ok());
}

#[test]
fn merging_many_members_into_a_single_line_container_stays_inline() {
    let source = HjsonDocument::parse("{\n  b: 2\n  c: 3\n}\n").unwrap();
    let host = HjsonDocument::parse("{a: 1}\n").unwrap();

    let merged = host.merge_from(&empty(), &source).unwrap();

    assert_eq!(merged.to_string(), "{a: 1, b: 2, c: 3}\n");
}

#[test]
fn merging_into_a_braceless_document_without_a_final_newline() {
    let host = HjsonDocument::parse("a: 1").unwrap();
    let source = HjsonDocument::parse("b: 2\nc: 3\n").unwrap();

    let merged = host.merge_from(&empty(), &source).unwrap();

    assert_eq!(merged.to_string(), "a: 1\nb: 2\nc: 3");
}

#[test]
fn merging_nested_objects_keeps_both_sides() {
    let mut host_body = String::new();
    let mut source_body = String::new();
    for index in 0..100 {
        host_body.push_str(&format!("    a{index}: {index}\n"));
        source_body.push_str(&format!("    b{index}: {index}\n"));
    }

    let host = HjsonDocument::parse(&format!("{{\n  o: {{\n{host_body}  }}\n}}\n")).unwrap();
    let source = HjsonDocument::parse(&format!("{{\n  o: {{\n{source_body}  }}\n}}\n")).unwrap();
    let merged = host.merge_from(&empty(), &source).unwrap();

    assert_eq!(merged.get(path("o.a99")).unwrap(), HjsonValue::Integer(99));
    assert_eq!(merged.get(path("o.b99")).unwrap(), HjsonValue::Integer(99));
    assert!(HjsonDocument::parse(&merged.to_string()).is_ok());
}

#[test]
fn merging_copies_comments_of_new_members_in_one_batch() {
    let source = HjsonDocument::parse("{\n  // lead\n  a: 1 // inline\n  b: 2\n}\n").unwrap();
    let host = HjsonDocument::parse("{\n  keep: 1\n}\n").unwrap();

    let merged = host.merge_from(&empty(), &source).unwrap();

    assert_eq!(
        merged.to_string(),
        "{\n  keep: 1\n  // lead\n  a: 1 // inline\n  b: 2\n}\n"
    );
}
