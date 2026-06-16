use hjson_edit::{
    high_level::{
        ops::object::HjsonDocumentObjectEdit,
        path::HjsonObjectPath,
        HjsonDocument,
    },
};

// ------------------------------------------------------------------
// ルートレベル配列に対する操作（get/set/remove/push）のテスト
// ------------------------------------------------------------------

#[test]
fn get_root_array_item_by_index() {
    let doc = HjsonDocument::parse("[1, 2, 3]").unwrap();
    let value = doc.get(HjsonObjectPath::parse("[0]").unwrap()).unwrap();
    assert!(
        matches!(value, hjson_edit::high_level::value::HjsonValue::Integer(1)),
        "expected Integer(1), got {:?}",
        value
    );
}

#[test]
fn set_root_array_item_by_index() {
    let doc = HjsonDocument::parse("[1, 2, 3]").unwrap();
    let updated = doc.set(HjsonObjectPath::parse("[1]").unwrap(), 4).unwrap();
    assert_eq!(updated.to_string(), "[1, 4, 3]");
}

#[test]
fn remove_root_array_last_item_no_trailing_comma() {
    let doc = HjsonDocument::parse("[1, 2, 3]").unwrap();
    let updated = doc.remove(HjsonObjectPath::parse("[-]").unwrap()).unwrap();
    assert_eq!(updated.to_string(), "[1, 2,]");
}

#[test]
fn set_push_on_root_array_appends() {
    let doc = HjsonDocument::parse("[1, 2]").unwrap();
    let updated = doc.set(HjsonObjectPath::parse("[-]").unwrap(), 3).unwrap();
    assert_eq!(updated.to_string(), "[1, 2, 3]");
}
