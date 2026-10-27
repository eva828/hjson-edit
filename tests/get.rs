use hjson_edit::high_level::{
    ops::object::HjsonDocumentEdit, path::HjsonObjectPath, HjsonDocument,
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

#[test]
fn duplicate_keys_keep_the_first_position_and_the_last_value() {
    let document = HjsonDocument::parse("{\n  a: 1\n  b: 2\n  a: 3\n}\n").unwrap();
    let value = document.get(HjsonObjectPath::parse("").unwrap()).unwrap();

    let hjson_edit::high_level::value::HjsonValue::Object(entries) = value else {
        panic!("expected an object");
    };
    let keys: Vec<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, ["a", "b"]);
    assert_eq!(
        entries[0].1,
        hjson_edit::high_level::value::HjsonValue::Integer(3)
    );
    assert_eq!(
        entries[1].1,
        hjson_edit::high_level::value::HjsonValue::Integer(2)
    );
}

#[test]
fn many_unique_keys_keep_the_source_order() {
    let mut text = String::from("{\n");
    for index in 0..2000 {
        text.push_str(&format!("  k{index}: {index}\n"));
    }
    text.push_str("}\n");

    let document = HjsonDocument::parse(&text).unwrap();
    let hjson_edit::high_level::value::HjsonValue::Object(entries) =
        document.get(HjsonObjectPath::parse("").unwrap()).unwrap()
    else {
        panic!("expected an object");
    };

    assert_eq!(entries.len(), 2000);
    for (index, (key, value)) in entries.iter().enumerate() {
        assert_eq!(key, &format!("k{index}"));
        assert_eq!(
            value,
            &hjson_edit::high_level::value::HjsonValue::Integer(index as i64)
        );
    }
}
