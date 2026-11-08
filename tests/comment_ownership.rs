//! 整形の前後で値とコメントが変わっていないことの検証。

use hjson_edit::HjsonDocument;

#[test]
fn verify_formatting_rejects_a_lost_comment() {
    let previous = HjsonDocument::parse("{\n  // keep\n  a: 1\n}\n").unwrap();
    let updated = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();

    assert!(updated.verify_formatting(&previous).is_err());
}

#[test]
fn verify_formatting_accepts_a_layout_change() {
    let previous = HjsonDocument::parse("{a: 1}\n").unwrap();
    let updated = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();

    updated.verify_formatting(&previous).unwrap();
}
