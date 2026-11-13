//! 重複キーの警告（値の層では後勝ち）。

use hjson_edit::high_level::ops::object::HjsonDocumentEdit;
use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::{HjsonDocument, HjsonValue};

#[test]
fn duplicate_keys_are_reported_as_warnings() {
    let doc = HjsonDocument::parse("{\n  a: 1\n  a: 2\n}\n").unwrap();

    assert_eq!(doc.warnings().len(), 1, "{:?}", doc.warnings());
    assert!(doc.warnings()[0].contains("duplicate key \"a\""));
    assert!(doc.warnings()[0].contains("line 3, column 3"));
    assert_eq!(
        doc.get(HjsonObjectPath::parse("a").unwrap()).unwrap(),
        HjsonValue::Integer(2)
    );
}

#[test]
fn duplicate_keys_in_a_braceless_document_are_reported() {
    let doc = HjsonDocument::parse("a: 1\na: 2\n").unwrap();

    assert_eq!(doc.warnings().len(), 1, "{:?}", doc.warnings());
    assert!(doc.warnings()[0].contains("line 2, column 1"));
}

#[test]
fn duplicate_keys_in_nested_objects_are_reported() {
    let doc = HjsonDocument::parse("{\n  o: {\n    x: 1\n    x: 2\n  }\n}\n").unwrap();

    assert_eq!(doc.warnings().len(), 1, "{:?}", doc.warnings());
    assert!(doc.warnings()[0].contains("duplicate key \"x\""));
    assert!(doc.warnings()[0].contains("line 4, column 5"));
}

#[test]
fn unique_keys_are_not_reported() {
    let doc = HjsonDocument::parse("{\n  a: 1\n  b: 2\n  o: { c: 3 }\n}\n").unwrap();

    assert!(doc.warnings().is_empty(), "{:?}", doc.warnings());
}

#[test]
fn an_unterminated_block_comment_warning_keeps_its_position() {
    let doc = HjsonDocument::parse("{\n  a: 1\n}\n/* x\n").unwrap();

    assert_eq!(doc.warnings().len(), 1, "{:?}", doc.warnings());
    assert!(doc.warnings()[0].contains("skipped input at line 4, column 1"));
}

#[test]
fn removing_a_duplicate_key_removes_every_occurrence() {
    for (src, expected) in [
        ("{a: 1, a: 2, b: 3}\n", "{b: 3}\n"),
        ("{a: 1, a: 2}\n", "{}\n"),
        ("{a: 1, a: 2, a: 3}\n", "{}\n"),
        ("{a: 1, a: 2, b: 3, a: 4}\n", "{b: 3,}\n"),
        ("{b: 1, a: 2, a: 3}\n", "{b: 1,}\n"),
        ("{a: 1, b: 2, a: 3}\n", "{b: 2,}\n"),
        ("{\n  a: 1\n  a: 2\n  b: 3\n}\n", "{\n  b: 3\n}\n"),
        ("a: 1\na: 2\nb: 3\n", "b: 3\n"),
    ] {
        let doc = HjsonDocument::parse(src).unwrap();
        let updated = doc
            .remove(HjsonObjectPath::parse("a").unwrap())
            .unwrap_or_else(|error| panic!("{src:?}: {error}"));

        assert_eq!(updated.to_string(), expected, "{src:?}");
    }
}

#[test]
fn removing_a_duplicate_key_keeps_the_other_values() {
    let doc = HjsonDocument::parse("{a: 1, a: 2, b: 3}\n").unwrap();
    let updated = doc.remove(HjsonObjectPath::parse("a").unwrap()).unwrap();

    assert_eq!(
        updated.get(HjsonObjectPath::parse("b").unwrap()).unwrap(),
        HjsonValue::Integer(3)
    );
    assert!(updated.get(HjsonObjectPath::parse("a").unwrap()).is_err());
}

#[test]
fn removing_a_duplicate_key_does_not_panic_on_three_occurrences() {
    // 範囲が重なったまま削除すると範囲外アクセスになる
    let doc = HjsonDocument::parse("{a: 1, a: 2, a: 3}\n").unwrap();

    let updated = doc.remove(HjsonObjectPath::parse("a").unwrap()).unwrap();

    assert_eq!(updated.to_string(), "{}\n");
}
