//! 同じ行にあるコメントの所有。
//!
//! カンマの後ろや、次の要素と同じ行で終わる複数行ブロックコメントは、前の要素の
//! 行末コメントとして 1 回だけ出力する（整形で二重に出さない）。

use hjson_edit::conformance::unescape;
use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::HjsonDocument;

fn format(input: &str) -> String {
    let doc = HjsonDocument::parse(input).unwrap();
    let config = doc.detected_format_config();
    doc.format_container(&HjsonObjectPath::parse("").unwrap(), &config)
        .unwrap()
        .to_string()
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

#[test]
fn comment_after_a_comma_is_emitted_once() {
    for (input, comment) in [
        ("[1, /* c */ 2]\n", "/* c */"),
        ("{a: 1, /* c */ b: 2}\n", "/* c */"),
        ("[1, // c\n  2]\n", "// c"),
        ("{a: 1, # c\n  b: 2}\n", "# c"),
    ] {
        let formatted = format(input);
        assert_eq!(
            count(&formatted, comment),
            1,
            "input {input:?} -> {formatted:?}"
        );
    }
}

#[test]
fn multiline_block_comment_between_members_is_emitted_once() {
    for (input, comment) in [
        ("ab: 1 /* x\nab/*b*/c: null\n", "/* x"),
        (
            "1: x\na//b: 1 /* unterminated\na/*b*/c:\nnull\n",
            "/* unterminated",
        ),
    ] {
        let formatted = format(input);
        assert_eq!(
            count(&formatted, comment),
            1,
            "input {input:?} -> {formatted:?}"
        );
    }
}

#[test]
fn trailing_comment_after_a_comma_is_kept() {
    assert_eq!(format("{a: 1, /* c */}\n"), "{\n  a: 1 /* c */\n}\n");
    assert_eq!(format("[1, /* c */]\n"), "[\n  1 /* c */\n]\n");
}

#[test]
fn formatting_is_idempotent_for_these_documents() {
    for input in [
        "[1, /* c */ 2]\n",
        "{a: 1, /* c */ b: 2}\n",
        "{a: 1, /* c */}\n",
        "ab: 1 /* x\nab/*b*/c: null\n",
    ] {
        let once = format(input);
        assert_eq!(format(&once), once, "input {input:?}");
    }
}

/// conformance の入力（パースできるもの）は、整形でコメントが変わらない。
#[test]
fn conformance_inputs_keep_their_comments_when_formatted() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hjson-cs/inputs.tsv");
    let text = std::fs::read_to_string(path).unwrap();
    let mut checked = 0;

    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut columns = line.split('\t');
        let id = columns.next().unwrap();
        let input = unescape(columns.next().unwrap());
        let Ok(doc) = HjsonDocument::parse(&input) else {
            continue;
        };

        let config = doc.detected_format_config();
        let result = doc.format_container(&HjsonObjectPath::parse("").unwrap(), &config);
        assert!(result.is_ok(), "{id}: {:?} -> {:?}", input, result.err());
        checked += 1;
    }

    assert!(checked > 80, "checked only {checked} inputs");
}

#[test]
fn verify_formatting_rejects_a_lost_comment() {
    let previous = HjsonDocument::parse("{\n  // keep\n  a: 1\n}\n").unwrap();
    let updated = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();

    assert!(updated.verify_formatting(&previous).is_err());
}

#[test]
fn verify_formatting_accepts_a_layout_change() {
    let previous = HjsonDocument::parse("{a: 1}\n").unwrap();
    let updated = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();

    updated.verify_formatting(&previous).unwrap();
}
