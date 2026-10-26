//! 複数行文字列（`'''`）が整形で値を変えないこと。

use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::{FormatConfig, HjsonDocument, HjsonDocumentEdit, HjsonValue, IndentStyle};

fn path(raw: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(raw).unwrap()
}

fn empty() -> HjsonObjectPath {
    HjsonObjectPath::from_segments(Vec::new())
}

/// 本文を `'''` の桁（列 5）に合わせて書いた文書。
///
/// 読み戻すと `body` になる。
fn document_with_block(body: &str) -> String {
    let mut text = String::from("{\n  s: '''\n");
    for line in body.split('\n') {
        if !line.is_empty() {
            text.push_str("     "); // `'''` の桁
            text.push_str(line);
        }
        text.push('\n');
    }
    text.push_str("  '''\n  a: 1\n}\n");

    text
}

fn value_of(document: &HjsonDocument) -> HjsonValue {
    document.get(path("s")).unwrap()
}

#[test]
fn the_document_builder_round_trips_the_body() {
    for body in ["line1\nline2", "  indented", "", "a\n\nb", "\ttab"] {
        let document = HjsonDocument::parse(&document_with_block(body)).unwrap();
        assert_eq!(
            value_of(&document),
            HjsonValue::String(body.to_string()),
            "body: {body:?}"
        );
    }
}

#[test]
fn formatting_the_document_keeps_block_string_values() {
    let bodies = [
        "line1\nline2",
        "  indented",
        "    deep\n  shallow",
        "a\n\nb",
        "trailing spaces   ",
        "日本語\nテキスト",
    ];
    let configs = [
        FormatConfig::default(),
        FormatConfig::default().with_indent(IndentStyle::Spaces(4)),
        FormatConfig::default().with_indent(IndentStyle::Tabs),
        FormatConfig::default().with_newline("\r\n"),
    ];

    for body in bodies {
        let document = HjsonDocument::parse(&document_with_block(body)).unwrap();
        for config in &configs {
            let formatted = document.format_container(&empty(), config).unwrap();

            assert_eq!(
                value_of(&formatted),
                HjsonValue::String(body.to_string()),
                "body: {body:?}, config: {config:?}"
            );
            assert!(HjsonDocument::parse(&formatted.to_string()).is_ok());
        }
    }
}

#[test]
fn formatting_a_container_keeps_block_string_values() {
    // 文書の字下げ（2）と合わない字下げ（6）を持つコンテナを整形しても値は変わらない
    let input = "{\n  a: 1\n  o: {\n      s: '''\n        deep\n      '''\n  }\n}\n";
    let document = HjsonDocument::parse(input).unwrap();
    let config = document.detected_format_config();

    let formatted = document.format_container(&path("o"), &config).unwrap();

    assert_eq!(
        formatted.get(path("o.s")).unwrap(),
        HjsonValue::String("deep".to_string())
    );
}

#[test]
fn the_slot_formatter_keeps_block_string_values() {
    let document = HjsonDocument::parse("{\n  s: '''x'''\n  a: 1\n}\n").unwrap();
    let replaced = document
        .set(path("s"), HjsonValue::String("line1\nline2".to_string()))
        .unwrap();
    let config = replaced.current_format_config();

    let formatted = replaced.format_slot(&path("s"), &config).unwrap();

    assert_eq!(
        formatted.get(path("s")).unwrap(),
        HjsonValue::String("line1\nline2".to_string())
    );
    assert_eq!(
        formatted.to_string(),
        "{\n  s: '''\n    line1\n    line2\n  '''\n  a: 1\n}\n"
    );
}

#[test]
fn the_slot_formatter_leaves_single_line_elements_alone() {
    let document = HjsonDocument::parse("{\n  a: {x: 1}\n}\n").unwrap();
    let config = document.current_format_config();

    let formatted = document.format_slot(&path("a"), &config).unwrap();

    assert_eq!(formatted.to_string(), document.to_string());
}

#[test]
fn replacing_a_block_value_with_a_single_line_keeps_the_block_style() {
    let document = HjsonDocument::parse("{\n  s: '''x'''\n}\n").unwrap();
    let replaced = document
        .set(path("s"), HjsonValue::String("plain".to_string()))
        .unwrap();

    assert_eq!(replaced.to_string(), "{\n  s: '''plain'''\n}\n");
}

#[test]
fn formatting_a_shallow_member_keeps_the_value() {
    // 文書の字下げ（推定 2）より浅いメンバーでも、実際の桁に合わせて書く
    let document = HjsonDocument::parse("{\na: '''x\ny'''\n}\n").unwrap();

    let formatted = document
        .format_slot(&path("a"), &document.current_format_config())
        .unwrap();

    assert_eq!(
        formatted.get(path("a")).unwrap(),
        HjsonValue::String("x\ny".to_string())
    );
    assert!(HjsonDocument::parse(&formatted.to_string()).is_ok());
}

#[test]
fn formatting_an_inline_member_leaves_it_alone() {
    let document = HjsonDocument::parse("{a: 1}\n").unwrap();
    let replaced = HjsonDocument::parse("{a: \'\'\'x\ny\'\'\'}\n").unwrap();
    let formatted = replaced
        .format_slot(&path("a"), &document.current_format_config())
        .unwrap();

    // 行頭でない要素は触らない（レイアウトを崩さない）
    assert_eq!(formatted.to_string(), replaced.to_string());
}

#[test]
fn writing_a_block_string_at_a_line_start_matches_the_indent() {
    let document = HjsonDocument::parse("{\n  a: \'\'\'x\'\'\'\n}\n").unwrap();
    let replaced = document
        .set(path("a"), HjsonValue::String("line1\nline2".to_string()))
        .unwrap();

    // set の時点では本文の字下げが無いので、スロット整形で揃える
    let formatted = replaced
        .format_slot(&path("a"), &replaced.current_format_config())
        .unwrap();

    assert_eq!(
        formatted.to_string(),
        "{\n  a: \'\'\'\n    line1\n    line2\n  \'\'\'\n}\n"
    );
}

#[test]
fn the_slot_formatter_formats_every_duplicate_key() {
    // 同じキーのメンバーが複数ある場合、値の層は後勝ちだが整形はすべてに適用する
    let text = "{\n  a: '''\n   one\n  '''\n  a: '''\n   two\n  '''\n}\n";
    let document = HjsonDocument::parse(text).unwrap();

    let formatted = document
        .format_slot(&path("a"), &document.current_format_config())
        .unwrap();

    assert_eq!(
        formatted.to_string(),
        "{\n  a: '''\n    one\n  '''\n  a: '''\n    two\n  '''\n}\n"
    );
}

#[test]
fn the_first_line_whitespace_of_a_block_string_is_ignored() {
    // 仕様: "whitespace on the first line is ignored"。cs と hjson-js も同じ
    for (input, expected) in [
        ("'''  x'''", "x"),
        ("'''  a: b'''", "a: b"),
        ("''' x '''", "x "),
        ("'''x '''", "x "),
        ("  '''  a: b'''", "a: b"),
        ("{ s: '''  a: b''' }", "a: b"),
        ("'''x'''", "x"),
    ] {
        let document = HjsonDocument::parse(input).unwrap();
        let value = document.get(path("s")).or_else(|_| {
            // ルートがスカラーの場合は空パスで読む
            document.get(path(""))
        });

        assert_eq!(
            value.unwrap(),
            HjsonValue::String(expected.to_string()),
            "input: {input:?}"
        );
    }
}

#[test]
fn a_trailing_blank_line_survives_formatting() {
    // 閉じ `'''` がカラム 0 にあると、読み戻しで末尾の改行が 1 つ落ちる。
    // 本文の末尾の空行を保つため、整形は改行を 1 つ余分に出す。
    for body in ["a", "a\n", "a\n\n", "\n a", "a\n\n\n"] {
        let input = document_with_block(body);
        let document = HjsonDocument::parse(&input).unwrap();
        let config = document.detected_format_config();

        let formatted = document
            .format_container(&HjsonObjectPath::parse("").unwrap(), &config)
            .unwrap();

        assert_eq!(
            value_of(&formatted),
            HjsonValue::String(body.to_string()),
            "body: {body:?}"
        );
    }
}

#[test]
fn a_trailing_blank_line_survives_slot_formatting() {
    let document = HjsonDocument::parse("{\n  s: '''x'''\n}\n").unwrap();
    let replaced = document
        .set(path("s"), HjsonValue::String("line\n\n".to_string()))
        .unwrap();

    let formatted = replaced
        .format_slot(&path("s"), &replaced.current_format_config())
        .unwrap();

    assert_eq!(
        formatted.get(path("s")).unwrap(),
        HjsonValue::String("line\n\n".to_string())
    );
}

#[test]
fn a_braceless_document_with_a_trailing_blank_line_formats() {
    // 翻訳ファイルと同じ形（ルートがブレースレス、閉じ `'''` がカラム 0）
    let input = "key:\n\t'''\n\tline1\n\tline2\n\n\n\t'''\nother: 1\n";
    let document = HjsonDocument::parse(input).unwrap();

    let formatted = document
        .format_container(
            &HjsonObjectPath::parse("").unwrap(),
            &document.current_format_config(),
        )
        .unwrap();

    assert_eq!(
        formatted.get(path("key")).unwrap(),
        document.get(path("key")).unwrap()
    );
    assert_eq!(
        formatted.get(path("other")).unwrap(),
        HjsonValue::Integer(1)
    );
}
