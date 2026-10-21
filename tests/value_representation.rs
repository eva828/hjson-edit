//! 文字列・数値の表現可能性（`set` が壊れた文書を作らないこと）。

use hjson_edit::ast::QuoteStyle;
use hjson_edit::high_level::error::HjsonEditError;
use hjson_edit::{HjsonDocument, HjsonDocumentEdit, HjsonObjectPath, HjsonValue, Prefer};

fn path(path: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(path).unwrap()
}

/// 引用符なし主体（HJSON で最も普通の文書）。
const UNQUOTED_DOCUMENT: &str = "{\n  name: my-app\n  version: 1.0.0\n}\n";

fn set_in_unquoted_document(value: HjsonValue) -> HjsonDocument {
    HjsonDocument::parse(UNQUOTED_DOCUMENT)
        .unwrap()
        .set(path("added"), value)
        .unwrap()
}

#[test]
fn strings_that_look_like_other_types_keep_their_type() {
    for text in ["true", "false", "null", "1.5", "-1", "[1]", "{a: 1}"] {
        let value = HjsonValue::String(text.to_string());
        let edited = set_in_unquoted_document(value.clone());

        assert_eq!(edited.get(path("added")).unwrap(), value, "text: {text:?}");
    }
}

#[test]
fn strings_that_need_quotes_are_quoted() {
    for text in [
        "", " a", "a ", "a\nb", "a\r\nb", "/* x */", "'''x", "#x", "//x",
    ] {
        let value = HjsonValue::String(text.to_string());
        let edited = set_in_unquoted_document(value.clone());

        assert_eq!(edited.get(path("added")).unwrap(), value, "text: {text:?}");
        assert!(
            HjsonDocument::parse(&edited.to_string()).is_ok(),
            "text: {text:?}"
        );
    }
}

#[test]
fn strings_that_round_trip_unquoted_stay_unquoted() {
    for text in ["hello world", "a\tb", "日本語", "a #b", "a:b", "x-y_z.w"] {
        let edited = set_in_unquoted_document(HjsonValue::String(text.to_string()));

        assert!(
            edited.to_string().contains(&format!("added: {text}")),
            "text: {text:?}, document: {:?}",
            edited.to_string()
        );
    }
}

#[test]
fn nested_strings_are_checked_too() {
    let value = HjsonValue::Object(vec![
        (
            "a".to_string(),
            HjsonValue::String("line\nbreak".to_string()),
        ),
        (
            "list".to_string(),
            HjsonValue::Array(vec![HjsonValue::String("true".to_string())]),
        ),
    ]);
    let edited = set_in_unquoted_document(value.clone());

    assert_eq!(edited.get(path("added")).unwrap(), value);
}

#[test]
fn an_explicit_style_that_cannot_represent_the_value_is_an_error() {
    let document = HjsonDocument::parse(UNQUOTED_DOCUMENT)
        .unwrap()
        .string_style(Prefer::Explicit(QuoteStyle::Unquoted));
    let error = document.set(path("added"), "line\nbreak").unwrap_err();

    assert!(
        matches!(error, HjsonEditError::ValueNotRepresentable { .. }),
        "unexpected error: {error:?}"
    );
}

#[test]
fn an_explicit_style_that_can_represent_the_value_is_kept() {
    let document = HjsonDocument::parse(UNQUOTED_DOCUMENT)
        .unwrap()
        .string_style(Prefer::Explicit(QuoteStyle::Unquoted));
    let edited = document.set(path("added"), "plain text").unwrap();

    assert!(edited.to_string().contains("added: plain text"));
}

#[test]
fn preserved_block_style_falls_back_to_quotes_when_the_body_cannot_be_block() {
    let document = HjsonDocument::parse("{\n  a: '''x'''\n}\n").unwrap();

    let kept = document.clone().set(path("a"), "line1\nline2").unwrap();
    assert!(
        kept.to_string().contains("a: '''line1"),
        "document: {:?}",
        kept.to_string()
    );

    for text in ["  a\n  b", "a'''b"] {
        let edited = document.clone().set(path("a"), text).unwrap();
        assert_eq!(
            edited.get(path("a")).unwrap(),
            HjsonValue::String(text.to_string())
        );
        assert!(
            edited.to_string().contains("a: \""),
            "document: {:?}",
            edited.to_string()
        );
    }
}

#[test]
fn non_finite_floats_are_rejected() {
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let error = HjsonDocument::parse(UNQUOTED_DOCUMENT)
            .unwrap()
            .set(path("added"), HjsonValue::Float(number))
            .unwrap_err();

        assert!(
            matches!(error, HjsonEditError::ValueNotRepresentable { .. }),
            "unexpected error: {error:?}"
        );
    }
}

#[test]
fn finite_floats_keep_their_value() {
    for number in [0.0, -0.0, 1.0, 1e16, 1e-7, f64::MAX, f64::MIN_POSITIVE] {
        let edited = set_in_unquoted_document(HjsonValue::Float(number));

        assert_eq!(
            edited.get(path("added")).unwrap(),
            HjsonValue::Float(number),
            "number: {number}"
        );
    }
}

/// 決定的な疑似乱数（xorshift64*）。
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }
}

#[test]
fn arbitrary_strings_either_round_trip_or_are_rejected() {
    // 文字列の判定（can_render）と実際の再パースが食い違わないことを確かめる。
    const ALPHABET: [&str; 24] = [
        "a", "Z", "0", "1", " ", "\t", "\n", "\r", "#", "/", "*", "'", "\"", "{", "[", ":", "true",
        "-", "\\", "\\u{7f}", "日", "]", "}", ",",
    ];

    let mut rng = Rng::new(0x5eed_1234);
    for _ in 0..500 {
        let length = rng.below(6);
        let mut text = String::new();
        for _ in 0..length {
            text.push_str(ALPHABET[rng.below(ALPHABET.len())]);
        }

        let document = HjsonDocument::parse(UNQUOTED_DOCUMENT).unwrap();
        match document.set(path("added"), HjsonValue::String(text.clone())) {
            Ok(edited) => {
                assert_eq!(
                    edited.get(path("added")).unwrap(),
                    HjsonValue::String(text.clone()),
                    "value changed for {text:?}"
                );
                assert!(
                    HjsonDocument::parse(&edited.to_string()).is_ok(),
                    "unparsable document for {text:?}: {:?}",
                    edited.to_string()
                );
            }
            Err(HjsonEditError::ValueNotRepresentable { .. }) => {}
            Err(error) => panic!("unexpected error for {text:?}: {error:?}"),
        }
    }
}

#[test]
fn arbitrary_values_keep_a_preserved_block_style_safe() {
    // 既存の複数行文字列に値を差し替えても、書けない場合は引用符へ切り替わる。
    const ALPHABET: [&str; 12] = [
        "a", " ", "\n", "\t", "'", "\"", "#", "{", "]", "日", "-", ",",
    ];

    let document = HjsonDocument::parse("{\n  a: \'\'\'x\'\'\'\n}\n").unwrap();
    let mut rng = Rng::new(0xb10c_0001);
    for _ in 0..200 {
        let length = rng.below(5);
        let mut text = String::new();
        for _ in 0..length {
            text.push_str(ALPHABET[rng.below(ALPHABET.len())]);
        }

        match document
            .clone()
            .set(path("a"), HjsonValue::String(text.clone()))
        {
            Ok(edited) => {
                assert_eq!(
                    edited.get(path("a")).unwrap(),
                    HjsonValue::String(text.clone()),
                    "value changed for {text:?}"
                );
            }
            Err(HjsonEditError::ValueNotRepresentable { .. }) => {}
            Err(error) => panic!("unexpected error for {text:?}: {error:?}"),
        }
    }
}
