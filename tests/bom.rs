use hjson_edit::HjsonDocument;

// ------------------------------------------------------------------
// RFC 8259 Section 9.1:
// "In the interests of interoperability, implementations that parse
//  Hjson texts MAY ignore the presence of a byte order mark."
// Current implementation treats BOM as an unexpected character.
// ------------------------------------------------------------------

#[test]
fn bom_at_start_is_ignored() {
    let input = "\u{FEFF}{\n  a: 1\n}\n";
    let result = HjsonDocument::parse(input);
    assert!(
        result.is_ok(),
        "BOM-prefixed input should be ignored per RFC, but got error: {:?}",
        result.err()
    );
}
