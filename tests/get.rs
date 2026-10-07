use hjson_edit::high_level::{
    ops::object::HjsonDocumentObjectEdit, path::HjsonObjectPath, HjsonDocument,
};

#[test]
fn get_existing_object_value_returns_string_value() {
    let input = "{\n  name: app\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc.get(HjsonObjectPath::parse("name").unwrap()).unwrap();
    assert!(matches!(value, hjson_edit::high_level::value::HjsonValue::String(s) if s == "app"));
}

#[test]
fn get_nonexistent_key_errors_path_not_found() {
    let input = "{\n  name: app\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let result = doc.get(HjsonObjectPath::parse("missing").unwrap());
    assert!(result.is_err());
}

#[test]
fn get_nested_path_returns_string_value() {
    let input = "{\n  server: { host: localhost\n}\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc
        .get(HjsonObjectPath::parse("server.host").unwrap())
        .unwrap();
    assert!(
        matches!(value, hjson_edit::high_level::value::HjsonValue::String(s) if s == "localhost")
    );
}

#[test]
fn get_array_index_returns_integer_value() {
    let input = "{\n  items: [1, 2, 3]\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc
        .get(HjsonObjectPath::parse("items[0]").unwrap())
        .unwrap();
    assert!(matches!(
        value,
        hjson_edit::high_level::value::HjsonValue::Integer(1)
    ));
}

#[test]
fn get_array_push_index_returns_last_integer_value() {
    let input = "{\n  items: [1, 2, 3]\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc
        .get(HjsonObjectPath::parse("items[-]").unwrap())
        .unwrap();
    assert!(matches!(
        value,
        hjson_edit::high_level::value::HjsonValue::Integer(3)
    ));
}

#[test]
fn get_array_middle_index_returns_integer_value() {
    let input = "{\n  items: [1, 2, 3]\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc
        .get(HjsonObjectPath::parse("items[1]").unwrap())
        .unwrap();
    assert!(matches!(
        value,
        hjson_edit::high_level::value::HjsonValue::Integer(2)
    ));
}

#[test]
fn get_root_scalar_returns_integer_value() {
    let input = "123\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    assert!(matches!(
        value,
        hjson_edit::high_level::value::HjsonValue::Integer(123)
    ));
}

#[test]
fn get_root_braceless_object_value_returns_integer_value() {
    let input = "name: app\nport: 8080\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc.get(HjsonObjectPath::parse("port").unwrap()).unwrap();
    assert!(matches!(
        value,
        hjson_edit::high_level::value::HjsonValue::Integer(8080)
    ));
}

#[test]
fn get_deeply_nested_path_returns_string_value() {
    let input = "{\n  a: { b: { c: value\n} }\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc.get(HjsonObjectPath::parse("a.b.c").unwrap()).unwrap();
    assert!(matches!(value, hjson_edit::high_level::value::HjsonValue::String(s) if s == "value"));
}

#[test]
fn get_array_then_object_path_returns_string_value() {
    let input = "{\n  items: [{ name: a\n}, { name: b\n}]\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc
        .get(HjsonObjectPath::parse("items[0].name").unwrap())
        .unwrap();
    assert!(matches!(value, hjson_edit::high_level::value::HjsonValue::String(s) if s == "a"));
}

#[test]
fn get_value_with_brackets() {
    let input = "{ spawn: x[i:CalamityEntropy/ProphecyToken]y\n}";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc.get(HjsonObjectPath::parse("spawn").unwrap()).unwrap();
    assert!(
        matches!(value, hjson_edit::high_level::value::HjsonValue::String(s) if s == "x[i:CalamityEntropy/ProphecyToken]y")
    );
}

#[test]
fn get_value_with_braces() {
    let input = "{ value: a{1}b\n}";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc.get(HjsonObjectPath::parse("value").unwrap()).unwrap();
    assert!(matches!(value, hjson_edit::high_level::value::HjsonValue::String(s) if s == "a{1}b"));
}
