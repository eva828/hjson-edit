use hjson_edit::HjsonDocument;

fn check_roundtrip(src: &str) {
    let doc = HjsonDocument::parse(src).expect("Failed to parse");
    assert_eq!(doc.to_string(), src, "Roundtrip failed");
}

#[test]
fn roundtrip_braceless_root() {
    check_roundtrip("foo: bar\n  baz: 123");
}

#[test]
fn roundtrip_mixed_comments() {
    let src = r#"
# Hash comment
// Line comment
/* Block
   Comment */
{
  "key": "value" // Inline comment
}
"#;
    check_roundtrip(src);
}

#[test]
fn roundtrip_block_strings() {
    let src = "{\n  doc: '''\n    Line 1\n    Line 2\n  '''\n}";
    check_roundtrip(src);
}

#[test]
fn roundtrip_mixed_newlines() {
    // string with \n and \r\n mixed
    let src = "{\n  foo: 1,\r\n  bar: 2\n}";
    check_roundtrip(src);
}

#[test]
fn roundtrip_escape_sequences() {
    let src = r#"{ "escaped": "\"\\\/\b\f\n\r\t\u1234" }"#;
    check_roundtrip(src);
}
