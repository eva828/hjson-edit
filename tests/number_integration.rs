use hjson_edit::high_level::{
    ops::object::HjsonDocumentObjectEdit, path::HjsonObjectPath, value::HjsonValue, HjsonDocument,
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
fn root_number_with_trailing_dot_is_a_string() {
    let doc = HjsonDocument::parse("1.").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(text) => assert_eq!(text, "1."),
        ref other => panic!("expected String(\"1.\"), got {:?}", other),
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
fn root_number_over_i64_max_returns_double() {
    // 基準実装（hjson-cs）は long で読めない整数を double として扱う。
    let doc = HjsonDocument::parse("9223372036854775808").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    assert_eq!(value, HjsonValue::Float(9223372036854775808.0));
}

#[test]
fn root_number_under_i64_min_returns_double() {
    let doc = HjsonDocument::parse("-9223372036854775809").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    assert_eq!(value, HjsonValue::Float(-9223372036854775809.0));
}

#[test]
fn root_number_large_exponent_does_not_panic() {
    let src = "1e99999999999999999999999999999999999999";
    let doc = HjsonDocument::parse(src).unwrap();
    let _ = doc.get(HjsonObjectPath::parse("").unwrap());
}

#[test]
fn integer_that_does_not_fit_i64_is_read_as_double() {
    // 基準実装（hjson-cs）は long で読めない整数を double として扱う。
    let doc = HjsonDocument::parse("x: 12345678901234567890123\n").unwrap();
    let value = doc.get(HjsonObjectPath::parse("x").unwrap()).unwrap();
    match value {
        HjsonValue::Float(f) => assert_eq!(f, 1.2345678901234568e22),
        other => panic!("expected Float, got {other:?}"),
    }
}
