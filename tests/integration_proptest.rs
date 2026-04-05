//! Property-based roundtrip tests

use hjson_edit::{HjsonDocument, HjsonDocumentObjectEdit};
use proptest::prelude::*;

proptest! {
    #[test]
    fn valid_object_roundtrip(
        pairs in prop::collection::vec(
            ("[a-z_][a-z0-9_]{1,10}", "[a-zA-Z0-9][a-zA-Z0-9 ]{0,19}"),
            0..10
        )
    ) {
        let obj = if pairs.is_empty() {
            String::from("{}")
        } else {
            let mut s = String::from("{\n");
            for (i, (k, v)) in pairs.iter().enumerate() {
                if i > 0 {
                    s.push('\n');
                }
                s.push_str(&format!("  {}: {}", k, v));
            }
            s.push_str("\n}");
            s
        };

        let doc = HjsonDocument::parse(&obj).unwrap();
        let output = doc.to_string();
        let reparsed = HjsonDocument::parse(&output).unwrap();
        prop_assert_eq!(output, reparsed.to_string());
    }

    #[test]
    fn valid_array_roundtrip(
        items in prop::collection::vec("[0-9]{1,5}", 0..20)
    ) {
        let arr = format!("[{}]", items.join(", "));

        let doc = HjsonDocument::parse(&arr).unwrap();
        let output = doc.to_string();
        let reparsed = HjsonDocument::parse(&output).unwrap();
        prop_assert_eq!(output, reparsed.to_string());
    }

    /// 引用符なし文字列がパンクチュエータで始まらないこと
    #[test]
    #[ignore = "AST operations not yet implemented"]
    fn quoteless_string_never_starts_with_punctuator(
        s in "[a-zA-Z_][a-zA-Z0-9_]{0,20}"
    ) {
        let input = format!("key: {}\n", s);
        let doc = HjsonDocument::parse(&input).unwrap();
        prop_assert!(doc.get("key").is_ok());
    }
}

#[test]
fn deep_nesting_roundtrip() {
    let mut input = String::new();
    for _ in 0..50 {
        input.push_str("{ nested: ");
    }
    input.push_str("\"value\"");
    for _ in 0..50 {
        input.push_str(" }");
    }

    let doc = HjsonDocument::parse(&input).unwrap();
    let output = doc.to_string();
    let reparsed = HjsonDocument::parse(&output).unwrap();
    assert_eq!(output, reparsed.to_string());
}

#[test]
fn unicode_roundtrip() {
    let cases = [
        r#"{ text: "こんにちは世界" }"#,
        r#"{ emoji: "🎉🎊🎁" }"#,
        r#"{ mixed: "Hello 世界 🌍" }"#,
        r#"{ arabic: "مرحبا" }"#,
        r#"{ russian: "Привет мир" }"#,
    ];

    for input in cases {
        let doc = HjsonDocument::parse(input).unwrap();
        let output = doc.to_string();
        let reparsed = HjsonDocument::parse(&output).unwrap();
        assert_eq!(
            output,
            reparsed.to_string(),
            "Unicode roundtrip failed for: {}",
            input
        );
    }
}
