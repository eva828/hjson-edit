use hjson_edit::{
    high_level::{
        ops::object::HjsonDocumentObjectEdit,
        path::HjsonObjectPath,
        value::HjsonValue,
        HjsonDocument,
    },
};

// ------------------------------------------------------------------
// These tests expect try-page behavior for hash (#) comments.
// try page: # is a comment ONLY at the start of a line.
// Anywhere else, # is part of the quoteless string value.
// These FAIL with the current implementation.
// ------------------------------------------------------------------

#[test]
fn hash_in_quoteless_string_is_part_of_value() {
    // try: value is "value # comment" (the # is part of the string)
    // current: value is "value" (# comment becomes a comment)
    let doc = HjsonDocument::parse("key: value # comment").unwrap();
    let value = doc.get(HjsonObjectPath::parse("key").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "value # comment"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn hash_in_quoteless_key_is_part_of_key() {
    // try: key is "key#value", value is 1
    // current: lex error (unexpected character: #)
    let doc = HjsonDocument::parse("key#value: 1").unwrap();
    let value = doc.get(HjsonObjectPath::parse("key#value").unwrap()).unwrap();
    match value {
        HjsonValue::Integer(1) => {}
        ref other => panic!("expected Integer(1), got {:?}", other),
    }
}

#[test]
fn hash_in_braceless_root_is_part_of_scalar_string() {
    // try: braceless root parses the ENTIRE line as a scalar string
    //      (no key/value split; everything is one unquoted string)
    // current: lex error because # is treated as comment start
    let doc = HjsonDocument::parse("key# value : 1").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "key# value : 1"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn hash_at_line_start_is_comment() {
    // try: # at start of line is a comment
    // current: same behavior
    let doc = HjsonDocument::parse("# comment\nkey: value").unwrap();
    let value = doc.get(HjsonObjectPath::parse("key").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "value"),
        ref other => panic!("expected String, got {:?}", other),
    }
}
