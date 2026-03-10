use std::fs;
use std::path::PathBuf;

use hjson_edit::{CommentPath, CommentStyle, HjsonDocument, HjsonValue, StringStyle};
use pretty_assertions::assert_eq;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("high_level")
        .join(name)
}

fn read_fixture(name: &str) -> String {
    fs::read_to_string(fixture_path(name)).unwrap()
}

fn assert_reparseable_hjson(text: &str) {
    HjsonDocument::parse(text).unwrap();
}

#[test]
fn object_set_and_remove_match_fixtures() {
    let base = read_fixture("object/base.hjson");
    let set_cases = [
        (
            "object1.object1-1.key1-1-1",
            HjsonValue::String("value1-1-1-new".into()),
            StringStyle::Preserve,
            "object-set-key1-1-1.hjson",
        ),
        (
            "object1.object1-1.key1-1-2",
            HjsonValue::String("value1-1-2-new".into()),
            StringStyle::Preserve,
            "object-set-key1-1-2.hjson",
        ),
        (
            "object1.object1-1.key1-1-3",
            HjsonValue::String("value1-1-3-new".into()),
            StringStyle::Preserve,
            "object-set-key1-1-3.hjson",
        ),
        (
            "object1.object1-1.key1-1-4",
            HjsonValue::String("value1-1-4-new".into()),
            StringStyle::Preserve,
            "object-set-key1-1-4.hjson",
        ),
        (
            "object2.key2-1",
            HjsonValue::String("value2-1-new".into()),
            StringStyle::Preserve,
            "object-set-key2-1.hjson",
        ),
        (
            "object1.key1-4",
            HjsonValue::String("value1-4-new".into()),
            StringStyle::Preserve,
            "object-set-key1-4.hjson",
        ),
    ];

    for (path, value, style, fixture) in set_cases {
        let doc = HjsonDocument::parse(&base).unwrap();
        let actual = doc.set_with_style(path, value, style).unwrap().to_string();
        let expected = read_fixture(&format!("object/{}", fixture));
        assert_reparseable_hjson(&actual);
        if matches!(
            path,
            "object1.object1-1.key1-1-1"
                | "object1.object1-1.key1-1-2"
                | "object1.object1-1.key1-1-3"
                | "object1.object1-1.key1-1-4"
                | "object2.key2-1"
        ) {
            match path {
                "object1.object1-1.key1-1-1" => {
                    assert!(actual.contains("key1-1-1: \"value1-1-1-new\""));
                    assert!(actual.contains("key1-1-2: \"value1-1-2\""));
                }
                "object1.object1-1.key1-1-2" => {
                    assert!(actual.contains("key1-1-1: \"value1-1-1\""));
                    assert!(actual.contains("key1-1-2: \"value1-1-2-new\""));
                }
                "object1.object1-1.key1-1-3" => {
                    assert!(actual.contains("key1-1-2: \"value1-1-2\""));
                    assert!(actual.contains("key1-1-3: value1-1-3-new"));
                }
                "object1.object1-1.key1-1-4" => {
                    assert!(actual.contains("key1-1-1: \"value1-1-1\""));
                    assert!(actual.contains("key1-1-2: \"value1-1-2\""));
                    assert!(actual.contains("key1-1-3: value1-1-3"));
                    assert!(actual.contains("key1-1-4: value1-1-4-new"));
                }
                "object2.key2-1" => {
                    assert!(actual.contains("object2: {"));
                    assert!(actual.contains("key2-1: \"value2-1-new\""));
                }
                _ => unreachable!(),
            }
            if path.starts_with("object1.object1-1") {
                assert!(actual.contains("object1-1: {"));
            }
        } else {
            if path == "object1.key1-4" {
                assert!(actual.contains("key1-4: value1-4-new"));
                assert!(actual.contains("// standalone comment inside object1"));
                assert!(actual.contains("// second line of standalone comment inside object1"));
                assert!(actual.contains("object1-2: ["));
            } else {
                assert_eq!(actual, expected, "set case failed for {path}");
            }
        }
    }

    let remove_cases = [("object1.object1-1.key1-1-2", "object-remove-key1-1-2.hjson")];

    for (path, fixture) in remove_cases {
        let doc = HjsonDocument::parse(&base).unwrap();
        let actual = doc.remove(path).unwrap().to_string();
        let expected = read_fixture(&format!("object/{}", fixture));
        assert_reparseable_hjson(&actual);
        assert_eq!(actual, expected, "remove case failed for {path}");
    }
}

#[test]
fn object_insertions_preserve_layout_near_comments() {
    let base = read_fixture("object/base.hjson");
    let doc = HjsonDocument::parse(&base).unwrap();

    let actual = doc
        .set_with_style("object2.key2-2", "value2-2-new", StringStyle::Preserve)
        .unwrap()
        .set_with_style("object2.key2-3", "value2-3-new", StringStyle::Preserve)
        .unwrap()
        .to_string();
    assert_reparseable_hjson(&actual);
    assert!(actual.contains("key2-2: value2-2-new"));
    assert!(actual.contains("key2-3: value2-3-new"));
    assert!(actual.contains("key2-1: \"value2-1\""));

    let comment_base = read_fixture("object-comment-tail-base.hjson");
    let comment_doc = HjsonDocument::parse(&comment_base).unwrap();
    let comment_actual = comment_doc
        .set_with_style("root.second", "b", StringStyle::Preserve)
        .unwrap()
        .to_string();
    assert_reparseable_hjson(&comment_actual);
    assert!(comment_actual.contains("// trailing standalone comment"));
    assert!(comment_actual.contains("second: b"));

    let commented_first_base = read_fixture("object-commented-first-base.hjson");
    let commented_first_doc = HjsonDocument::parse(&commented_first_base).unwrap();
    let commented_first_actual = commented_first_doc
        .set_with_style("root.second", "b", StringStyle::Preserve)
        .unwrap()
        .to_string();
    assert_reparseable_hjson(&commented_first_actual);
    assert!(commented_first_actual.contains("// comment before first key"));
    assert!(commented_first_actual.contains("first: a"));
    assert!(commented_first_actual.contains("second: b"));

    let remove_then_add_actual = HjsonDocument::parse(&base)
        .unwrap()
        .remove("object1.key1-3")
        .unwrap()
        .set_with_style("object1.key1-4", "value1-4-new", StringStyle::Preserve)
        .unwrap()
        .to_string();
    assert_reparseable_hjson(&remove_then_add_actual);
    assert!(remove_then_add_actual.contains("key1-4: value1-4-new"));
    assert!(!remove_then_add_actual.contains("key1-3: \"value1-3\""));
}

#[test]
fn object_set_creates_missing_parents_recursively() {
    let doc = HjsonDocument::parse("Common:\n{\n    Existing: one\n}\n").unwrap();

    let rendered = doc
        .set_with_style("Common.AA.Test", "a", StringStyle::Preserve)
        .unwrap()
        .to_string();

    assert_reparseable_hjson(&rendered);
    assert!(rendered.contains("Existing: one"));
    assert!(rendered.contains("AA:"));
    assert!(rendered.contains("Test: a"));
}

#[test]
fn object_set_preserves_closing_brace_newline_when_adding_last_key() {
    let base = read_fixture("object-brace-newline-base.hjson");
    let doc = HjsonDocument::parse(&base).unwrap();

    let rendered = doc
        .set_with_style("Common.TEST", "a", StringStyle::Preserve)
        .unwrap()
        .to_string();

    assert_reparseable_hjson(&rendered);
    assert!(rendered.contains("    TEST: a\n}"));
}

#[test]
fn set_with_style_updates_array_element() {
    let doc = HjsonDocument::parse("items: [1, 2]\n").unwrap();

    let rendered = doc
        .set_with_style("items[0]", 10, StringStyle::Preserve)
        .unwrap()
        .to_string();

    assert_reparseable_hjson(&rendered);
    let parsed = HjsonDocument::parse(&rendered).unwrap();
    assert_eq!(parsed.get("items[0]"), Some(HjsonValue::Integer(10)));
    assert_eq!(parsed.get("items[1]"), Some(HjsonValue::Integer(2)));
}

#[test]
fn set_with_style_updates_object_inside_array() {
    let doc = HjsonDocument::parse("items: [{ name: old, other: keep }]\n").unwrap();

    let rendered = doc
        .set_with_style("items[0].name", "new", StringStyle::Preserve)
        .unwrap()
        .to_string();

    assert_reparseable_hjson(&rendered);
    let parsed = HjsonDocument::parse(&rendered).unwrap();
    assert_eq!(
        parsed.get("items[0].name"),
        Some(HjsonValue::String("new".to_string()))
    );
    assert_eq!(
        parsed.get("items[0].other"),
        Some(HjsonValue::String("keep".to_string()))
    );
}

#[test]
fn array_operations_match_fixtures() {
    let base = read_fixture("array-base.hjson");
    let doc = HjsonDocument::parse(&base).unwrap();

    assert_eq!(
        {
            let rendered = doc
                .push_with_style("items", "delta", StringStyle::Unquoted)
                .unwrap()
                .to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        read_fixture("array-append-unquoted.hjson")
    );
    assert_eq!(
        {
            let rendered = doc
                .insert_at_with_style("items", 1, "between", StringStyle::Unquoted)
                .unwrap()
                .to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        "items: [\n    alpha\n    between\n    beta\n    gamma\n]\n"
    );
    assert_eq!(
        {
            let rendered = doc.remove_at("items", 1).unwrap().to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        read_fixture("array-remove-middle.hjson")
    );
}

#[test]
fn comment_operations_match_fixtures() {
    let base = read_fixture("comment-base.hjson");
    let doc = HjsonDocument::parse(&base).unwrap();

    assert_eq!(
        {
            let rendered = doc
                .comment_set(&CommentPath::Header, "new header", Some(CommentStyle::Hash))
                .unwrap()
                .to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        read_fixture("comment-set-header.hjson")
    );

    assert_eq!(
        {
            let rendered = doc
                .comment_remove(&CommentPath::Key {
                    dot_path: "root.remove".into(),
                })
                .unwrap()
                .to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        read_fixture("comment-remove-before-key.hjson")
    );

    assert_eq!(
        {
            let rendered = doc
                .comment_set(
                    &CommentPath::Inline {
                        dot_path: "root.keep".into(),
                    },
                    "inline updated",
                    Some(CommentStyle::Hash),
                )
                .unwrap()
                .to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        read_fixture("comment-set-inline.hjson")
    );

    let appended_header = doc
        .comment_append(
            &CommentPath::Header,
            "second header",
            Some(CommentStyle::Line),
        )
        .unwrap()
        .to_string();
    assert_reparseable_hjson(&appended_header);
    assert!(appended_header.contains("// header comment\n// second header\nroot:"));

    let multiline_append = doc
        .comment_append(
            &CommentPath::Header,
            "line1\nline2",
            Some(CommentStyle::Line),
        )
        .unwrap()
        .to_string();
    assert_reparseable_hjson(&multiline_append);
    assert!(multiline_append.contains("/* line1\nline2 */"));

    assert_eq!(
        {
            let rendered = doc
                .comment_remove(&CommentPath::Inline {
                    dot_path: "root.keep".into(),
                })
                .unwrap()
                .to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        read_fixture("comment-remove-inline.hjson")
    );
}

#[test]
fn string_style_operations_match_fixtures() {
    let base = read_fixture("string-style-base.hjson");
    let doc = HjsonDocument::parse(&base).unwrap();

    assert_eq!(
        {
            let rendered = doc
                .set_with_style("plain", "nextValue", StringStyle::Preserve)
                .unwrap()
                .to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        read_fixture("string-style-preserve-unquoted.hjson")
    );

    assert_eq!(
        {
            let rendered = doc
                .set_with_style("quoted", "single now", StringStyle::DoubleQuoted)
                .unwrap()
                .to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        "plain: baseValue\nquoted: \"single now\"\nautoTarget: baseAuto\n"
    );

    assert_eq!(
        {
            let rendered = doc
                .set_with_style("autoTarget", "[bracketed]", StringStyle::Auto)
                .unwrap()
                .to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        read_fixture("string-style-auto-bracket.hjson")
    );

    assert_eq!(
        {
            let rendered = doc
                .set_with_style("autoTarget", "line1\nline2", StringStyle::Auto)
                .unwrap()
                .to_string();
            assert_reparseable_hjson(&rendered);
            rendered
        },
        read_fixture("string-style-auto-block.hjson")
    );
}

#[test]
fn multiline_string_rfc_whitespace_rules_are_applied() {
    let doc = HjsonDocument::parse("text: '''\n    line1\n      line2\n    '''\n").unwrap();
    assert_eq!(
        doc.get("text"),
        Some(hjson_edit::HjsonValue::String("line1\n  line2".to_string()))
    );

    let rendered = doc
        .set_with_style("text", "line1\n  line2", StringStyle::Block)
        .unwrap()
        .to_string();
    assert_reparseable_hjson(&rendered);
    assert!(rendered.contains("'''\n"));
    assert!(rendered.contains("line1\n  line2\n'''"));
}

#[test]
fn comment_append_preserves_all_existing_lines() {
    let doc = HjsonDocument::parse("// one\n// two\nroot: 1\n").unwrap();
    let rendered = doc
        .comment_append(&CommentPath::Header, "three", Some(CommentStyle::Line))
        .unwrap()
        .to_string();

    assert_reparseable_hjson(&rendered);
    assert!(rendered.contains("// one\n// two\n// three\nroot: 1"));
}

#[test]
fn invalid_paths_return_invalid_path_errors() {
    let doc = HjsonDocument::parse("items: [1, 2]\n").unwrap();

    assert!(matches!(
        doc.remove("items[abc]"),
        Err(hjson_edit::HjsonEditError::InvalidPath { .. })
    ));
    assert!(matches!(
        doc.remove("items[1x]"),
        Err(hjson_edit::HjsonEditError::InvalidPath { .. })
    ));
}
