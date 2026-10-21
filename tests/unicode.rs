use hjson_edit::high_level::{
    ops::object::HjsonDocumentEdit, path::HjsonObjectPath, value::HjsonValue, HjsonDocument,
};

#[test]
fn parse_unicode_key_and_value() {
    let doc = HjsonDocument::parse("日本語: 値").unwrap();
    let value = doc.get(HjsonObjectPath::parse("日本語").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "値"),
        ref other => panic!("expected String, got {:?}", other),
    }
}

#[test]
fn get_and_set_with_unicode_path() {
    let doc = HjsonDocument::parse("{\n  日本語: 値\n}\n").unwrap();
    let value = doc.get(HjsonObjectPath::parse("日本語").unwrap()).unwrap();
    match value {
        HjsonValue::String(ref s) => assert_eq!(s, "値"),
        ref other => panic!("expected String, got {:?}", other),
    }

    let updated = doc
        .set(HjsonObjectPath::parse("日本語").unwrap(), "新値")
        .unwrap();
    assert!(updated.to_string().contains("新値"));
}
