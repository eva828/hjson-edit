use hjson_edit::{FormatTarget, HjsonDocument};

fn assert_roundtrip(input: &str) {
    let document = HjsonDocument::parse(input).expect("input should parse");
    let rendered = document.to_string();

    assert_eq!(
        rendered, input,
        "rendering parsed document should preserve original text"
    );

    let reparsed = HjsonDocument::parse(&rendered).expect("rendered text should reparse");
    assert_eq!(
        reparsed.to_string(),
        rendered,
        "second render should remain stable"
    );
}

#[test]
fn roundtrip_preserves_comments_and_mixed_layout() {
    let input = concat!(
        "{\n",
        "  // header comment\n",
        "  server: {\n",
        "    host: localhost\n",
        "    port: 8080, // inline comment\n",
        "\n",
        "    paths: [\n",
        "      /api\n",
        "      /health\n",
        "    ]\n",
        "  }\n",
        "}\n",
    );

    assert_roundtrip(input);
}

#[test]
fn roundtrip_preserves_root_braceless_object() {
    let input = concat!(
        "# root header\n",
        "name: app\n",
        "enabled: true\n",
        "items:\n",
        "[\n",
        "  1\n",
        "  2\n",
        "]\n",
    );

    assert_roundtrip(input);
}

#[test]
fn format_document_roundtrip_remains_stable_after_reparse() {
    let input = concat!(
        "{\n",
        "  app:\n",
        "  {\n",
        "    name: app\n",
        "    features: [\n",
        "      alpha,\n",
        "      beta\n",
        "    ]\n",
        "  }\n",
        "}\n",
    );

    let document = HjsonDocument::parse(input).expect("input should parse");
    let formatted = document
        .format(vec![FormatTarget::Document])
        .expect("document formatting should succeed");
    let rendered = formatted.to_string();
    let reparsed = HjsonDocument::parse(&rendered).expect("formatted text should reparse");

    assert_eq!(
        reparsed.to_string(),
        rendered,
        "formatted document should remain stable after reparsing"
    );
}

#[test]
fn partial_format_roundtrip_remains_stable_after_reparse() {
    let input = concat!(
        "{\n",
        "  outer: {\n",
        "    items: [\n",
        "      {\n",
        "        name: first\n",
        "      },\n",
        "      {\n",
        "        name: second\n",
        "      }\n",
        "    ]\n",
        "  }\n",
        "}\n",
    );

    let document = HjsonDocument::parse(input).expect("input should parse");
    let formatted = document
        .format(vec![FormatTarget::path("outer.items")])
        .expect("partial formatting should succeed");
    let rendered = formatted.to_string();
    let reparsed = HjsonDocument::parse(&rendered).expect("formatted text should reparse");

    assert_eq!(
        reparsed.to_string(),
        rendered,
        "partial formatted output should remain stable after reparsing"
    );
}

#[test]
fn edit_then_format_roundtrip_remains_stable() {
    let input = concat!("{\n", "  service: {\n", "    name: app\n", "  }\n", "}\n",);

    let document = HjsonDocument::parse(input).expect("input should parse");
    let edited = document
        .set("service.name", "api")
        .expect("edit should succeed");

    let rendered = edited.to_string();
    let reparsed = HjsonDocument::parse(&rendered).expect("edited text should reparse");

    assert_eq!(
        reparsed.to_string(),
        rendered,
        "edited and formatted output should be stable after reparsing"
    );
}

#[test]
fn multiline_string_format_roundtrip_remains_stable() {
    let input = concat!(
        "{\n",
        "  message:\n",
        "    '''\n",
        "    first line\n",
        "      indented line\n",
        "    '''\n",
        "}\n",
    );

    let document = HjsonDocument::parse(input).expect("input should parse");
    let formatted = document
        .format(vec![FormatTarget::Document])
        .expect("formatting should succeed");

    let rendered = formatted.to_string();
    let reparsed = HjsonDocument::parse(&rendered).expect("formatted text should reparse");

    assert_eq!(
        reparsed.to_string(),
        rendered,
        "formatted multiline string output should remain stable"
    );
}
