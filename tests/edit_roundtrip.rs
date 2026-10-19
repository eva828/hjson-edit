//! 編集結果が常に再パース可能で、値が一致することを検証する。
//!
//! 引用符なし文字列は行末でしか終端しないため、挿入位置を誤ると `}`/`]`/`,` が
//! 値に吸収されて文書が壊れる。ここでは編集後のテキストを再パースして値が一致する
//! ことを、コンテナの形と値の種類の組み合わせごとに確認する。

use hjson_edit::high_level::{
    ops::object::HjsonDocumentObjectEdit, path::HjsonObjectPath, value::HjsonValue, HjsonDocument,
};
use hjson_edit::HjsonDocumentMerge;

/// 編集結果を再パースし、ルートの値が一致することを確認する。
fn assert_roundtrip(doc: &HjsonDocument) {
    let text = doc.to_string();
    let reparsed = HjsonDocument::parse(&text)
        .unwrap_or_else(|error| panic!("編集結果を再パースできない:\n{text}\n{error}"));
    let root = HjsonObjectPath::parse("").unwrap();
    assert_eq!(
        reparsed.get(root.clone()).unwrap(),
        doc.get(root).unwrap(),
        "再パース後の値が一致しない:\n{text}"
    );
}

fn set(input: &str, path: &str, value: impl Into<HjsonValue>) -> HjsonDocument {
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse(path).unwrap(), value)
        .unwrap();
    assert_roundtrip(&updated);
    updated
}

#[test]
fn append_unquoted_string_to_multiline_object() {
    assert_eq!(
        set("{\n  a: 1\n}\n", "b", "hello").to_string(),
        "{\n  a: 1\nb: hello\n}\n"
    );
}

#[test]
fn append_unquoted_string_to_single_line_object() {
    assert_eq!(
        set("{a: 1}", "b", "hello").to_string(),
        "{a: 1\nb: hello\n}"
    );
}

#[test]
fn append_number_keeps_single_line_object() {
    assert_eq!(set("{a: 1}", "b", 2).to_string(), "{a: 1, b: 2}");
}

#[test]
fn append_to_empty_object() {
    assert_eq!(set("{}", "name", "app").to_string(), "{\nname: app\n}");
    assert_eq!(set("{\n}", "name", "app").to_string(), "{\nname: app\n}");
    assert_eq!(set("{}", "count", 1).to_string(), "{count: 1}");
}

#[test]
fn push_unquoted_string_into_multiline_array() {
    assert_eq!(
        set("[\n  a\n]\n", "[-]", "nv").to_string(),
        "[\n  a\nnv\n]\n"
    );
}

#[test]
fn push_into_single_line_array() {
    assert_eq!(set("[1, 2]", "[-]", 3).to_string(), "[1, 2, 3]");
    assert_eq!(set("[1, 2]", "[-]", "three").to_string(), "[1, 2\nthree\n]");
}

#[test]
fn push_into_empty_array() {
    assert_eq!(set("[]", "[-]", "a").to_string(), "[\na\n]");
    assert_eq!(set("[\n]", "[-]", 1).to_string(), "[\n1\n]");
}

#[test]
fn insert_unquoted_string_in_the_middle() {
    assert_eq!(set("[1, 3]", "[+1]", "two").to_string(), "[1,two\n 3]");
    assert_eq!(set("[1, 2]", "[+0]", "zero").to_string(), "[zero\n1, 2]");
}

#[test]
fn append_to_nested_and_inline_containers() {
    assert_eq!(
        set("{o: {x: 1}}", "o.y", "two").to_string(),
        "{o: {x: 1\ny: two\n}}"
    );
    assert_eq!(
        set("{o: [1]}", "o[-]", "two").to_string(),
        "{o: [1\ntwo\n]}"
    );
    assert_eq!(
        set("[{a: 1}]", "[0].b", "two").to_string(),
        "[{a: 1\nb: two\n}]"
    );
}

#[test]
fn append_to_braceless_root() {
    assert_eq!(set("a: one", "b", "two").to_string(), "a: one\nb: two");
    assert_eq!(set("a: 1", "b", "two").to_string(), "a: 1, b: two");
    assert_eq!(set("a: one\n", "b", "two").to_string(), "a: one\nb: two\n");
}

#[test]
fn append_unquoted_string_keeps_crlf_document_parseable() {
    let doc = HjsonDocument::parse("{\r\n  a: 1\r\n}\r\n").unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("b").unwrap(), "hello")
        .unwrap();
    assert_roundtrip(&updated);
    assert_eq!(updated.to_string(), "{\r\n  a: 1\r\nb: hello\r\n}\r\n");
}

#[test]
fn set_container_value_ending_with_unquoted_string() {
    let value = HjsonValue::Object(vec![(
        "x".to_string(),
        HjsonValue::String("hello".to_string()),
    )]);
    let doc = set("{\n  a: 1\n}\n", "b", value.clone());
    assert_eq!(doc.to_string(), "{\n  a: 1\nb: {x:hello\n}\n}\n");
    let doc = set("{a: 1}", "b", value);
    assert_eq!(doc.to_string(), "{a: 1, b: {x:hello\n}}");
}

#[test]
fn set_nested_path_creates_all_parents() {
    // 先頭のキーが無い場合でも、途中のコンテナをまとめて作成する
    assert_eq!(set("{}\n", "a.b", 1).to_string(), "{a: {b:1}}\n");
    assert_eq!(
        set("{x: 1}\n", "a.b.c", "hello").to_string(),
        "{x: 1, a: {b:{c:hello\n}}}\n"
    );
}

#[test]
fn set_container_value_with_unquoted_string_before_another_element() {
    // 途中の要素が引用符なし文字列で終わる場合、カンマは値に吸収されるため改行で区切る
    let object = HjsonValue::Object(vec![
        ("x".to_string(), HjsonValue::String("hello".to_string())),
        ("y".to_string(), HjsonValue::Integer(1)),
    ]);
    assert_eq!(
        set("{a: 1}", "b", object).to_string(),
        "{a: 1, b: {x:hello\ny:1}}"
    );

    let array = HjsonValue::Array(vec![
        HjsonValue::String("hello".to_string()),
        HjsonValue::Integer(1),
    ]);
    assert_eq!(
        set("{a: 1}", "b", array).to_string(),
        "{a: 1, b: [hello\n1]}"
    );
}

#[test]
fn append_into_single_line_container_keeps_crlf() {
    let doc = HjsonDocument::parse("{\r\n  o: {a: 1}\r\n}\r\n").unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("o.b").unwrap(), "hello")
        .unwrap();
    assert_roundtrip(&updated);
    assert_eq!(
        updated.to_string(),
        "{\r\n  o: {a: 1\r\nb: hello\r\n}\r\n}\r\n"
    );
}

#[test]
fn merge_unquoted_string_member_is_parseable() {
    let host = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();
    let source = HjsonDocument::parse("{\n  b: two\n}\n").unwrap();
    let merged = host
        .merge_from(&HjsonObjectPath::parse("").unwrap(), &source)
        .unwrap();
    assert_roundtrip(&merged);
    assert_eq!(merged.to_string(), "{\n  a: 1\nb: two\n}\n");

    let host = HjsonDocument::parse("{}").unwrap();
    let source = HjsonDocument::parse("{\n  unq: plain\n}\n").unwrap();
    let merged = host
        .merge_from(&HjsonObjectPath::parse("").unwrap(), &source)
        .unwrap();
    assert_roundtrip(&merged);
}
