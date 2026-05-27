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
fn root_number_with_leading_zero_returns_string() {
    let doc = HjsonDocument::parse("01").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "01"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn root_number_with_trailing_dot_returns_float() {
    let doc = HjsonDocument::parse("1.").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::Float(v) => assert_eq!(v, 1.0),
        ref other => panic!("expected Float(1.0), got {:?}", other),
    }
}

#[test]
fn root_number_with_incomplete_exponent_returns_string() {
    let doc = HjsonDocument::parse("1e").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "1e"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn root_number_with_plus_sign_returns_string() {
    let doc = HjsonDocument::parse("+1").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "+1"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn root_number_hex_returns_string() {
    let doc = HjsonDocument::parse("0x1").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "0x1"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn root_number_bare_decimal_returns_string() {
    let doc = HjsonDocument::parse(".5").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, ".5"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn root_number_over_i64_max_returns_error() {
    let src = "9223372036854775808";
    let doc = HjsonDocument::parse(src).unwrap();
    let result = doc.get(HjsonObjectPath::parse("").unwrap());
    assert!(result.is_err());
}

#[test]
fn root_number_under_i64_min_returns_error() {
    let src = "-9223372036854775809";
    let doc = HjsonDocument::parse(src).unwrap();
    let result = doc.get(HjsonObjectPath::parse("").unwrap());
    assert!(result.is_err());
}

#[test]
fn root_number_large_exponent_does_not_panic() {
    let src = "1e99999999999999999999999999999999999999"; 
    let doc = HjsonDocument::parse(src).unwrap();
    let _ = doc.get(HjsonObjectPath::parse("").unwrap());
}
