use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::{HjsonDocument, HjsonDocumentEdit, HjsonValue};

fn path(raw: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(raw).unwrap()
}

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

#[test]
fn roundtrip_unquoted_with_bracket_in_value() {
    // C# readTfnns: `]` and `}` inside values are checkpoints consumed as content.
    // The closing `}` is absorbed → object never closes → parse error.
    assert!(HjsonDocument::parse("{ spawn: x[i:CalamityEntropy/ProphecyToken]y }").is_err());
}

#[test]
fn roundtrip_unquoted_with_brace_in_value() {
    // Same as above: `}` is absorbed.
    assert!(HjsonDocument::parse("{ value: a{1}b }").is_err());
}

#[test]
fn roundtrip_unquoted_with_bracket_braceless() {
    check_roundtrip("key: abc[xyz]def\n");
}

#[test]
fn roundtrip_unquoted_with_mixed_brackets() {
    // `}` inside value is absorbed, then closing `}` also absorbed → error.
    assert!(HjsonDocument::parse("{ data: pre[abc]{def}post }").is_err());
}

#[test]
fn whitespace_between_key_and_colon_is_preserved() {
    // キーと `:` の間の空白は読み飛ばさず、lossless な木に残す
    for text in [
        "{\n  s  : 1\n}\n",
        "{\n  s\t:\t1\n}\n",
        "{\n  \"q\"  : 1\n}\n",
        "{\n  s  : '''\n   x\n  '''\n}\n",
        "{\n  s  :\n    1\n}\n",
        "a  : 1\n",
    ] {
        let document = HjsonDocument::parse(text).unwrap();

        assert_eq!(document.to_string(), text);
    }
}

#[test]
fn editing_keeps_the_whitespace_before_the_colon() {
    let document = HjsonDocument::parse("{\n  s  : 1\n  t\t:\t2\n}\n").unwrap();
    let edited = document.set(path("s"), 9).unwrap();

    assert_eq!(edited.to_string(), "{\n  s  : 9\n  t\t:\t2\n}\n");
}

#[test]
fn a_newline_before_the_colon_is_preserved() {
    let text = "{\n  s  :\n    1\n}\n";
    let document = HjsonDocument::parse(text).unwrap();

    assert_eq!(document.to_string(), text);
    assert_eq!(document.get(path("s")).unwrap(), HjsonValue::Integer(1));
}
