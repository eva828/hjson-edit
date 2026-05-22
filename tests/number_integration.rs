use hjson_edit::{
    high_level::{
        ops::object::HjsonDocumentObjectEdit,
        path::HjsonObjectPath,
        value::HjsonValue,
        HjsonDocument,
    },
};

// ------------------------------------------------------------------
// Integration tests for number parsing at the document level.
// These verify the parsed value type, not just the classifier.
// ------------------------------------------------------------------

#[test]
fn parse_01_becomes_string() {
    // try page: 01 is treated as a string (leading zeros not allowed as number)
    let doc = HjsonDocument::parse("01").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "01"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn parse_1_dot_becomes_integer() {
    // User spec: "1." should be treated as integer 1 (try page behavior).
    // Current implementation returns None from classify, so lexer emits UnquotedString.
    let doc = HjsonDocument::parse("1.").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::Integer(1) => {}
        ref other => panic!("expected Integer(1), got {:?}", other),
    }
}

#[test]
fn parse_1e_becomes_string() {
    // try page: 1e is treated as a string (incomplete exponent)
    let doc = HjsonDocument::parse("1e").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "1e"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn parse_plus_1_becomes_string() {
    // RFC: plus sign is not allowed for numbers
    let doc = HjsonDocument::parse("+1").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "+1"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn parse_hex_becomes_string() {
    // RFC: hexadecimal is not allowed
    let doc = HjsonDocument::parse("0x1").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "0x1"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn parse_bare_decimal_becomes_string() {
    // RFC: .5 is not a valid number (must start with digit)
    let doc = HjsonDocument::parse(".5").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, ".5"),
        ref other => panic!("expected String, got {:?}", other),
    }
}
