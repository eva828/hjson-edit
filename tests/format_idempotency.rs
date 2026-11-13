//! 整形は 2 回かけても同じ結果になる（冪等）。
//!
//! 閉じていないブロックコメントは EOF までを飲み込むため、その後ろに改行を足すと
//! コメントの一部になり、整形のたびに改行が増えてしまう。

use hjson_edit::high_level::ops::object::HjsonDocumentEdit;
use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::HjsonDocument;

fn format(input: &str) -> String {
    let doc = HjsonDocument::parse(input).unwrap();
    let path = HjsonObjectPath::parse("").unwrap();
    doc.format_container(&path, &doc.detected_format_config())
        .unwrap()
        .to_string()
}

#[test]
fn an_unterminated_block_comment_does_not_grow_on_each_format() {
    // 改行はコメントの一部になるため、整形は後ろに何も足さない
    for (input, expected) in [
        ("x: 5/*c", "x: 5 /*c"),
        ("x: 5 /*c", "x: 5 /*c"),
        ("x: 5/*c\n", "x: 5 /*c\n"),
        ("x: 5 /*c\n", "x: 5 /*c\n"),
    ] {
        let once = format(input);

        assert_eq!(once, expected, "input {input:?}");
        assert_eq!(format(&once), once, "input {input:?}");
    }
}

#[test]
fn an_unterminated_comment_in_the_container_tail_does_not_grow() {
    // ブレースレス文書で、閉じていないブロックコメントが独立した行にある場合
    for (input, expected) in [
        ("a: 1\n/*c", "a: 1\n/*c"),
        ("a: 1\n/*c\n", "a: 1\n/*c\n"),
        ("a: 1\n\n/*c", "a: 1\n\n/*c"),
        ("a: 1\n//c\n/*c", "a: 1\n//c\n/*c"),
        ("{\n  a: 1\n}\n/*c", "{\n  a: 1\n}\n/*c"),
        ("a: 1\r\n/*c", "a: 1\r\n/*c"),
        // 字下げはコンテナの桁に揃える（コメントの本文は変えない）
        ("a: 1\n  /*c", "a: 1\n/*c"),
    ] {
        let once = format(input);

        assert_eq!(once, expected, "input {input:?}");
        assert_eq!(format(&once), once, "input {input:?}");
    }
}

#[test]
fn a_terminated_comment_still_ends_the_line() {
    for (input, expected) in [
        ("x: 5 //c", "x: 5 //c\n"),
        ("x: 5 #c", "x: 5 #c\n"),
        ("x: 5 /*c*/", "x: 5 /*c*/\n"),
        ("{a: 1}\n", "{\n  a: 1\n}\n"),
        ("a: 1\nb: 2\n", "a: 1\nb: 2\n"),
    ] {
        assert_eq!(format(input), expected, "input {input:?}");
    }
}

#[test]
fn formatting_keeps_the_values_and_comments_of_these_documents() {
    for input in [
        "x: 5/*c",
        "x: 5 //c",
        "x: 5 /*c*/",
        "[1, 2]/*c",
        "a: 1\n/*c",
        "a: 1\n//c\n/*c",
        "{a: 1, /* c */ b: 2}\n",
        "{\n  a: 1\n  // c\n}\n",
    ] {
        let doc = HjsonDocument::parse(input).unwrap();
        let formatted = format(input);
        let reparsed = HjsonDocument::parse(&formatted).unwrap();

        assert_eq!(reparsed.all_comments(), doc.all_comments(), "{input:?}");
        assert_eq!(
            reparsed.get(HjsonObjectPath::parse("").unwrap()).unwrap(),
            doc.get(HjsonObjectPath::parse("").unwrap()).unwrap(),
            "{input:?}"
        );
    }
}

/// フィクスチャの入力（解析できるもの）は、整形を 2 回かけても変わらない。
#[test]
fn conformance_inputs_are_formatted_idempotently() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hjson-cs/inputs.tsv");
    let text = std::fs::read_to_string(path).unwrap();
    let mut checked = 0;

    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let Some(escaped) = line.split('\t').nth(1) else {
            continue;
        };
        let input = hjson_edit::conformance::unescape(escaped);
        let Ok(doc) = HjsonDocument::parse(&input) else {
            continue;
        };
        let slot = HjsonObjectPath::parse("").unwrap();
        let Ok(once) = doc.format_container(&slot, &doc.detected_format_config()) else {
            continue;
        };
        checked += 1;

        let Ok(reparsed) = HjsonDocument::parse(&once.to_string()) else {
            panic!("{input:?}: formatted text does not parse: {once}");
        };
        let twice = reparsed
            .format_container(&slot, &reparsed.detected_format_config())
            .unwrap();

        assert_eq!(twice.to_string(), once.to_string(), "input {input:?}");
    }

    assert!(checked > 50, "checked {checked}");
}
