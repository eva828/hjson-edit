use hjson_edit::high_level::{
    ops::comment::HjsonDocumentCommentEdit, ops::object::HjsonDocumentEdit, path::HjsonObjectPath,
    value::HjsonValue, HjsonDocument,
};
use pretty_assertions::assert_eq;

#[test]
fn set_array_push_appends_to_end() {
    let input = "{\n  items: [1, 2, 3]\n}\n";
    let expected = "{\n  items: [1, 2, 3, 4]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[-]").unwrap(), 4)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_array_index_updates_existing_value() {
    let input = "{\n  items: [1, 2, 3]\n}\n";
    let expected = "{\n  items: [1, 4, 3]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[1]").unwrap(), 4)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_array_insert_at_past_end_fails() {
    let input = "{\n  items: [1, 2]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let result = doc.set(HjsonObjectPath::parse("items[+10]").unwrap(), 3);
    assert!(result.is_err());
}

#[test]
fn set_array_insert_at_len_appends() {
    let input = "{\n  items: [1, 2]\n}\n";
    let expected = "{\n  items: [1, 2, 3]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[+2]").unwrap(), 3)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_array_insert_at_inserts_in_middle() {
    let input = "{\n  items: [1, 3]\n}\n";
    let expected = "{\n  items: [1, 2, 3]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[+1]").unwrap(), 2)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_object_key_updates_existing_value() {
    let input = "{\n  name: app\n}\n";
    let expected = "{\n  name: api\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("name").unwrap(), "api")
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_nested_object_path_creates_member() {
    let input = "{\n  server: { host: localhost\n}\n}\n";
    let expected = "{\n  server: { host: localhost\nport: 8080\n}\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("server.port").unwrap(), 8080)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_missing_nested_path_creates_parents() {
    let input = "{\n  root: {\n  }\n}\n";
    // 追加した要素は、閉じ区切りと同じ字下げに 1 段足した位置へ入る
    let expected = "{\n  root: {\n    new: {nested: value\n}\n  }\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("root.new.nested").unwrap(), "value")
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_into_empty_object_inserts_member() {
    let input = "{\n}\n";
    let expected = "{\n  name: app\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("name").unwrap(), "app")
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_into_empty_array_inserts_item() {
    let input = "[\n]\n";
    let expected = "[\n  1\n]\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("[-]").unwrap(), 1).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_existing_key_preserves_leading_comments() {
    let input = "{\n  // keep this note\n  name: app\n}\n";
    let expected = "{\n  // keep this note\n  name: api\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("name").unwrap(), "api")
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn set_preserves_crlf_line_endings() {
    let input = "{\n  items: [\r\n    1\r\n    2\r\n  ]\n}\n";
    let expected = "{\n  items: [\r\n    1\r\n    2\r\n    3\r\n  ]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[-]").unwrap(), 3)
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn append_to_multiline_object() {
    let input = "{\n  a: 1\n}\n";
    let expected = "{\n  a: 1\n  b: 2\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("b").unwrap(), 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn append_to_multiline_array() {
    let input = "[\n  1\n]\n";
    let expected = "[\n  1\n  2\n]\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("[-]").unwrap(), 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn append_to_object_with_trailing_comment_preserves_comment() {
    // 末尾コメント（閉じ区切りの直前のコメント）は末尾に残し、新しいメンバーはその手前に入る
    let input = "{\n  a: 1\n  // trailing\n}\n";
    let expected = "{\n  a: 1\n  b: 2\n  // trailing\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("b").unwrap(), 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn append_to_object_with_inline_and_trailing_comments_preserves_both() {
    let input = "{\n  a: 1 // inline\n  // trailing\n}\n";
    let expected = "{\n  a: 1 // inline\n  b: 2\n  // trailing\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("b").unwrap(), 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn append_to_array_with_trailing_comment_preserves_comment() {
    let input = "[\n  1\n  // trailing\n]\n";
    let expected = "[\n  1\n  2\n  // trailing\n]\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.set(HjsonObjectPath::parse("[-]").unwrap(), 2).unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn remove_object_key_deletes_member() {
    let input = "{\n  server: { host: localhost,\n  port: 8080\n}\n}\n";
    let expected = "{\n  server: { host: localhost,\n}\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .remove(HjsonObjectPath::parse("server.port").unwrap())
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn remove_array_last_item_deletes_end_element() {
    let input = "{\n  items: [1, 2, 3]\n}\n";
    let expected = "{\n  items: [1, 2,]\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .remove(HjsonObjectPath::parse("items[-]").unwrap())
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

#[test]
fn remove_object_member_cleans_trivia_avoids_empty_lines() {
    let input = "{\n  a: aaa,\n  b: bbb\n  c: ccc\n  d: ddd\n}\n";
    let expected = "{\n  a: aaa,\n  b: bbb\n  d: ddd\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.remove(HjsonObjectPath::parse("c").unwrap()).unwrap();

    assert_eq!(updated.to_string(), expected);
}

#[test]
fn remove_last_object_member_preserves_trailing_comma() {
    let input = "{\n  keep: 1,\n  remove: 2\n}\n";
    let expected = "{\n  keep: 1,\n}\n";

    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc
        .remove(HjsonObjectPath::parse("remove").unwrap())
        .unwrap();
    assert_eq!(updated.to_string(), expected);
}

// ------------------------------------------------------------------
// 空値のメンバーへの設定（基準実装に合わせた欠陥修正の回帰テスト）
// ------------------------------------------------------------------

#[test]
fn empty_value_document_is_a_single_string() {
    // 値の無いブレースレスは基準実装（hjson-cs）と同じく行全体が 1 個の文字列になる。
    for input in ["key:", "key:\n", "key: //c"] {
        let doc = HjsonDocument::parse(input).unwrap();
        let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
        assert_eq!(value, HjsonValue::String(input.trim_end().to_string()));
    }
}

#[test]
fn set_on_string_root_is_rejected() {
    let doc = HjsonDocument::parse("key:").unwrap();
    let error = doc
        .set(HjsonObjectPath::parse("key").unwrap(), 1)
        .unwrap_err();
    assert!(
        matches!(
            error,
            hjson_edit::HjsonEditError::UnexpectedStructure { .. }
                | hjson_edit::HjsonEditError::PathNotFound(_)
                | hjson_edit::HjsonEditError::InvalidTarget { .. }
        ),
        "文字列ルートへの設定はエラーになる: {error:?}"
    );
}

/// 値を変えない `set` はテキストも変えない（体裁の正規化を起こさない）。
#[test]
fn set_with_an_equal_value_keeps_the_text() {
    for input in [
        "{\n  a: {x: 1, y: 2}\n}\n",
        "{\n  a: [1, 2]\n}\n",
        "{\n  a: { \"k\": 1 }\n}\n",
        "{\n  a: 1.0\n}\n",
        "{\n  a: 1e3\n}\n",
        "{\n  a:\n    {\n      x: 1\n    }\n}\n",
    ] {
        let doc = HjsonDocument::parse(input).unwrap();
        let value = doc.get(HjsonObjectPath::parse("a").unwrap()).unwrap();
        let updated = doc
            .set(HjsonObjectPath::parse("a").unwrap(), value)
            .unwrap();

        assert_eq!(updated.to_string(), input);
    }
}

#[test]
fn set_with_an_equal_number_keeps_the_float_text() {
    let doc = HjsonDocument::parse("{\n  a: 1.0\n}\n").unwrap();
    let updated = doc.set(HjsonObjectPath::parse("a").unwrap(), 1).unwrap();

    assert_eq!(updated.to_string(), "{\n  a: 1.0\n}\n");
}

#[test]
fn set_with_an_equal_array_item_keeps_the_text() {
    let input = "{\n  items: [{x: 1, y: 2}, 3]\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc
        .get(HjsonObjectPath::parse("items[0]").unwrap())
        .unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("items[0]").unwrap(), value)
        .unwrap();

    assert_eq!(updated.to_string(), input);
}

#[test]
fn set_with_an_equal_value_for_duplicate_keys_keeps_both() {
    let input = "{\n  a: {x: 1}\n  a: {x: 1}\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let value = doc.get(HjsonObjectPath::parse("a").unwrap()).unwrap();
    let updated = doc
        .set(HjsonObjectPath::parse("a").unwrap(), value)
        .unwrap();

    assert_eq!(updated.to_string(), input);
}

#[test]
fn set_with_an_equal_value_and_an_explicit_style_rewrites_the_value() {
    let doc = HjsonDocument::parse("{\n  a: x\n}\n")
        .unwrap()
        .string_style(hjson_edit::Prefer::Explicit(
            hjson_edit::ast::QuoteStyle::DoubleQuoted,
        ));
    let updated = doc
        .set(
            HjsonObjectPath::parse("a").unwrap(),
            HjsonValue::String("x".to_string()),
        )
        .unwrap();

    assert_eq!(updated.to_string(), "{\n  a: \"x\"\n}\n");
}

#[test]
fn verify_value_edits_accepts_a_matching_batch() {
    use hjson_edit::high_level::verify::ValueEdit;

    let original = HjsonDocument::parse("{\n  a: 1\n  b: 2\n}\n").unwrap();
    let edits = vec![
        ValueEdit::Set(HjsonObjectPath::parse("a").unwrap(), HjsonValue::Integer(9)),
        ValueEdit::Remove(HjsonObjectPath::parse("b").unwrap()),
    ];

    let mut doc = original.clone().verify_edits(false);
    doc = doc.set(HjsonObjectPath::parse("a").unwrap(), 9).unwrap();
    doc = doc.remove(HjsonObjectPath::parse("b").unwrap()).unwrap();

    doc.verify_value_edits(&original, &edits).unwrap();
}

#[test]
fn verify_value_edits_rejects_a_mismatched_value() {
    use hjson_edit::high_level::verify::ValueEdit;

    let original = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();
    let edits = vec![ValueEdit::Set(
        HjsonObjectPath::parse("a").unwrap(),
        HjsonValue::Integer(9),
    )];

    let doc = original
        .clone()
        .verify_edits(false)
        .set(HjsonObjectPath::parse("a").unwrap(), 8)
        .unwrap();
    let error = doc.verify_value_edits(&original, &edits).unwrap_err();

    assert!(matches!(
        error,
        hjson_edit::HjsonEditError::InvalidEdit { .. }
    ));
}

#[test]
fn verify_value_edits_rejects_a_lost_comment() {
    use hjson_edit::high_level::verify::ValueEdit;

    let original = HjsonDocument::parse("// keep\n{\n  a: 1\n}\n").unwrap();
    let edits = vec![ValueEdit::Set(
        HjsonObjectPath::parse("a").unwrap(),
        HjsonValue::Integer(2),
    )];

    // 値を変えつつ、編集対象の外にあるコメントを消した文書
    let doc = original
        .clone()
        .verify_edits(false)
        .set(HjsonObjectPath::parse("a").unwrap(), 2)
        .unwrap()
        .remove_comment(hjson_edit::high_level::path::HjsonCommentPath::Header)
        .unwrap();
    let error = doc.verify_value_edits(&original, &edits).unwrap_err();

    assert!(matches!(
        error,
        hjson_edit::HjsonEditError::InvalidEdit { .. }
    ));
}

/// 値テキストをそのまま差し替える（体裁を正規化しない）。
#[test]
fn set_value_text_keeps_the_text_as_written() {
    for (input, path, text) in [
        ("{\n  a: {x: 1, y: 2}\n}\n", "a", "{x: 1,  y: 2}"),
        ("{\n  a: 1\n}\n", "a", "1.0"),
        ("{\n  a: 1\n}\n", "a", "1e3"),
        ("{\n  a: 1\n}\n", "a", "{ \"k\": 1 }"),
        ("{\n  items: [1, 2]\n}\n", "items[0]", "2.50"),
    ] {
        let doc = HjsonDocument::parse(input).unwrap();
        let updated = doc
            .set_value_text(HjsonObjectPath::parse(path).unwrap(), text)
            .unwrap();

        assert!(
            updated.to_string().contains(text),
            "text: {text:?}, document: {:?}",
            updated.to_string()
        );
    }
}

#[test]
fn set_value_text_rejects_texts_that_cannot_be_placed_inline() {
    let doc = HjsonDocument::parse("{\n  a: 1\n  b: 2\n}\n").unwrap();
    for (path, text) in [
        ("a", "1\n2"),
        ("a", "a: 1"),
        ("a", "hello world"),
        ("a", "1 // comment"),
        ("missing", "1"),
    ] {
        let result = doc
            .clone()
            .set_value_text(HjsonObjectPath::parse(path).unwrap(), text);
        assert!(result.is_err(), "path: {path}, text: {text:?}");
    }
}

#[test]
fn set_value_text_replaces_the_document_body() {
    let doc = HjsonDocument::parse("a: 1\n").unwrap();
    let updated = doc
        .set_value_text(HjsonObjectPath::parse("").unwrap(), "5")
        .unwrap();

    // ルートの差し替えは set("") と同じ（本文ノードを置き換える）
    assert_eq!(updated.to_string(), "5");
}

#[test]
fn set_value_text_on_root_accepts_multiline_and_key_line_text() {
    let path = HjsonObjectPath::parse("").unwrap();

    let doc = HjsonDocument::parse("{\n  a: 1\n}\n").unwrap();
    let updated = doc.set_value_text(path.clone(), "a: 1\nb: 2").unwrap();
    assert_eq!(updated.to_string(), "a: 1\nb: 2\n");

    let doc = HjsonDocument::parse("a: 1\n").unwrap();
    let updated = doc.set_value_text(path, "{\n  x: 1\n}").unwrap();
    assert_eq!(updated.to_string(), "{\n  x: 1\n}");
}

#[test]
fn set_value_text_on_root_keeps_document_header_and_footer() {
    let path = HjsonObjectPath::parse("").unwrap();

    // ヘッダーは Root 直下なので残る
    let doc = HjsonDocument::parse("// header\na: 1\n").unwrap();
    let updated = doc.set_value_text(path.clone(), "5").unwrap();
    assert_eq!(updated.to_string(), "// header\n5");

    // 中括弧付き文書のフッターも Root 直下なので残る
    let doc = HjsonDocument::parse("{\n  a: 1\n}\n// f\n").unwrap();
    let updated = doc.set_value_text(path.clone(), "5").unwrap();
    assert_eq!(updated.to_string(), "5\n// f\n");

    // ブレースレス文書の末尾コメントは本文の内側なので、本文ごと置き換わる
    let doc = HjsonDocument::parse("a: 1\n// footer\n").unwrap();
    let updated = doc.set_value_text(path, "5").unwrap();
    assert_eq!(updated.to_string(), "5");
}

#[test]
fn set_value_text_retargets_a_missing_parent_to_the_requested_path() {
    let doc = HjsonDocument::parse("a: 1\n").unwrap();
    let requested = HjsonObjectPath::parse("a.b.c").unwrap();
    let error = doc.set_value_text(requested.clone(), "2").unwrap_err();

    match error {
        hjson_edit::HjsonEditError::PathNotFound(path) => assert_eq!(path, requested),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn set_value_text_reports_a_scalar_parent_as_invalid_target() {
    let doc = HjsonDocument::parse("a: 1\n").unwrap();
    let requested = HjsonObjectPath::parse("a.b").unwrap();
    let error = doc.set_value_text(requested.clone(), "2").unwrap_err();

    match error {
        hjson_edit::HjsonEditError::InvalidTarget { path, reason } => {
            assert_eq!(path, requested);
            assert!(reason.contains("scalar"), "reason: {reason}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn remove_keeps_the_inline_comment_of_the_previous_member() {
    for (input, expected) in [
        ("{a: 1, /* c */ b: 2}\n", "{a: 1, /* c */}\n"),
        ("{a: 1, /* c */\n  b: 2}\n", "{a: 1, /* c */\n}\n"),
        ("[1, /* c */ 2]\n", "[1, /* c */]\n"),
    ] {
        let doc = HjsonDocument::parse(input).unwrap();
        let path = if input.starts_with('[') { "[1]" } else { "b" };
        let updated = doc.remove(HjsonObjectPath::parse(path).unwrap()).unwrap();

        assert_eq!(updated.to_string(), expected, "{input:?}");
        assert_eq!(updated.all_comments(), vec!["c".to_string()], "{input:?}");
    }
}

#[test]
fn remove_deletes_the_before_comment_of_the_removed_member() {
    let input = "{\n  a: 1\n  /* c */\n  b: 2\n}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.remove(HjsonObjectPath::parse("b").unwrap()).unwrap();

    assert_eq!(updated.to_string(), "{\n  a: 1\n}\n");
    assert!(updated.all_comments().is_empty());
}

#[test]
fn remove_keeps_the_inline_comment_and_drops_the_before_comment() {
    let input = "{a: 1, /* x */\n  /* y */\n  b: 2}\n";
    let doc = HjsonDocument::parse(input).unwrap();
    let updated = doc.remove(HjsonObjectPath::parse("b").unwrap()).unwrap();

    assert_eq!(updated.to_string(), "{a: 1, /* x */\n}\n");
    assert_eq!(updated.all_comments(), vec!["x".to_string()]);
}
