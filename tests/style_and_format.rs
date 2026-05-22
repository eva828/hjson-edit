use hjson_edit::{
    ast::{CommentKind, QuoteStyle},
    high_level::{
        ops::object::HjsonDocumentObjectEdit,
        path::HjsonObjectPath,
        Prefer, HjsonDocument,
    },
};
use pretty_assertions::assert_eq;

#[test]
fn set_with_preserve_string_style_keeps_double_quotes() {
    let input = "{\n  name: \"app\"\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .string_style(Prefer::Preserve)
        .set(HjsonObjectPath::parse("name").unwrap(), "api")
        .unwrap();
    assert_eq!(updated.to_string(), "{\n  name: \"api\"\n}\n");
}

#[test]
fn set_with_explicit_double_quoted_renders_double_quotes() {
    let input = "{\n  name: \"app\"\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .string_style(Prefer::Explicit(QuoteStyle::DoubleQuoted))
        .set(HjsonObjectPath::parse("name").unwrap(), "api")
        .unwrap();
    assert_eq!(updated.to_string(), "{\n  name: \"api\"\n}\n");
}

#[test]
fn set_with_explicit_block_renders_block_quotes() {
    let input = "{\n  name: \"app\"\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .string_style(Prefer::Explicit(QuoteStyle::Block))
        .set(HjsonObjectPath::parse("name").unwrap(), "api")
        .unwrap();
    assert_eq!(updated.to_string(), "{\n  name: '''api'''\n}\n");
}

#[test]
fn set_with_explicit_unquoted_renders_without_quotes() {
    let input = "{\n  name: \"app\"\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .string_style(Prefer::Explicit(QuoteStyle::Unquoted))
        .set(HjsonObjectPath::parse("name").unwrap(), "api")
        .unwrap();
    assert_eq!(updated.to_string(), "{\n  name: api\n}\n");
}
