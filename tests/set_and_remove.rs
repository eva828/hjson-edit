use hjson_edit::high_level::{
    ops::object::HjsonDocumentObjectEdit, path::HjsonObjectPath, value::HjsonValue, HjsonDocument,
};
use pretty_assertions::assert_eq;

#[test]
fn set_array_push_appends_to_end() {
    let input = "{\n  items: [1, 2, 3]\n}\n";
    let expected = "{\n  items: [1, 2, 3, 4]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[-]").unwrap(), 4)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_array_index_updates_existing_value() {
    let input = "{\n  items: [1, 2, 3]\n}\n";
    let expected = "{\n  items: [1, 4, 3]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[1]").unwrap(), 4)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_array_insert_at_past_end_fails() {
    let input = "{\n  items: [1, 2]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let result = doc.set(HjsonObjectPath::parse("items[+10]").unwrap(), 3);
    assert!(result.is_err());
}

#[test]
fn set_array_insert_at_len_appends() {
    let input = "{\n  items: [1, 2]\n}\n";
    let expected = "{\n  items: [1, 2, 3]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[+2]").unwrap(), 3)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_array_insert_at_inserts_in_middle() {
    let input = "{\n  items: [1, 3]\n}\n";
    let expected = "{\n  items: [1,2, 3]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[+1]").unwrap(), 2)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_object_key_updates_existing_value() {
    let input = "{\n  name: app\n}\n";
    let expected = "{\n  name: api\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("name").unwrap(), "api")
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_nested_object_path_creates_member() {
    let input = "{\n  server: { host: localhost\n}\n}\n";
    let expected = "{\n  server: { host: localhost\nport: 8080\n}\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("server.port").unwrap(), 8080)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_missing_nested_path_creates_parents() {
    let input = "{\n  root: {\n  }\n}\n";
    let expected = "{\n  root: {\nnew: {nested:value\n}\n  }\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("root.new.nested").unwrap(), "value")
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_into_empty_object_inserts_member() {
    let input = "{\n}\n";
    let expected = "{\nname: app\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("name").unwrap(), "app")
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_into_empty_array_inserts_item() {
    let input = "[\n]\n";
    let expected = "[\n1\n]\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("[-]").unwrap(), 1).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_existing_key_preserves_leading_comments() {
    let input = "{\n  // keep this note\n  name: app\n}\n";
    let expected = "{\n  // keep this note\n  name: api\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("name").unwrap(), "api")
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_preserves_crlf_line_endings() {
    let input = "{\n  items: [\r\n    1\r\n    2\r\n  ]\n}\n";
    let expected = "{\n  items: [\r\n    1\r\n    2\r\n3\r\n  ]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[-]").unwrap(), 3)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn append_to_multiline_object() {
    let input = "{\n  a: 1\n}\n";
    let expected = "{\n  a: 1\nb: 2\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("b").unwrap(), 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn append_to_multiline_array() {
    let input = "[\n  1\n]\n";
    let expected = "[\n  1\n2\n]\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("[-]").unwrap(), 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn append_to_object_with_trailing_comment_preserves_comment() {
    let input = "{\n  a: 1\n  // trailing\n}\n";
    let expected = "{\n  a: 1\n  // trailing\nb: 2\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("b").unwrap(), 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn append_to_object_with_inline_and_trailing_comments_preserves_both() {
    let input = "{\n  a: 1 // inline\n  // trailing\n}\n";
    let expected = "{\n  a: 1 // inline\n  // trailing\nb: 2\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("b").unwrap(), 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn append_to_array_with_trailing_comment_preserves_comment() {
    let input = "[\n  1\n  // trailing\n]\n";
    let expected = "[\n  1\n  // trailing\n2\n]\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("[-]").unwrap(), 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn remove_object_key_deletes_member() {
    let input = "{\n  server: { host: localhost,\n  port: 8080\n}\n}\n";
    let expected = "{\n  server: { host: localhost,\n}\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .remove(HjsonObjectPath::parse("server.port").unwrap())
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn remove_array_last_item_deletes_end_element() {
    let input = "{\n  items: [1, 2, 3]\n}\n";
    let expected = "{\n  items: [1, 2,]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .remove(HjsonObjectPath::parse("items[-]").unwrap())
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn remove_object_member_cleans_trivia_avoids_empty_lines() {
    let input = "{\n  a: aaa,\n  b: bbb\n  c: ccc\n  d: ddd\n}\n";
    let expected = "{\n  a: aaa,\n  b: bbb\n  d: ddd\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.remove(HjsonObjectPath::parse("c").unwrap()).unwrap();

    assert_eq!(updated.to_string(), expected);
}

#[test]
fn remove_last_object_member_preserves_trailing_comma() {
    let input = "{\n  keep: 1,\n  remove: 2\n}\n";
    let expected = "{\n  keep: 1,\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .remove(HjsonObjectPath::parse("remove").unwrap())
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

// ------------------------------------------------------------------
// 空値のメンバーへの設定（基準実装に合わせた欠陥修正の回帰テスト）
// ------------------------------------------------------------------

#[test]
fn empty_value_document_is_a_single_string() {
    // 値の無いブレースレスは基準実装（hjson-cs）と同じく行全体が 1 個の文字列になる。
    for input in ["key:", "key:\n", "key: //c"] {
        let doc = HjsonDocument::parse(input).unwrap();
        let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
        assert_eq!(value, HjsonValue::String(input.trim_end().to_string()));
    }
}

#[test]
fn set_on_string_root_is_rejected() {
    let doc = HjsonDocument::parse("key:").unwrap();
    let error = doc
        .set(HjsonObjectPath::parse("key").unwrap(), 1)
        .unwrap_err();
    assert!(
        matches!(
            error,
            hjson_edit::HjsonEditError::UnexpectedStructure { .. }
                | hjson_edit::HjsonEditError::PathNotFound(_)
                | hjson_edit::HjsonEditError::ParentNotFound
        ),
        "文字列ルートへの設定はエラーになる: {error:?}"
    );
}
