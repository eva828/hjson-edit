use hjson_edit::high_level::{
    ops::merge::HjsonDocumentMerge, ops::object::HjsonDocumentEdit, path::HjsonObjectPath,
    HjsonDocument,
};
use hjson_edit::MAX_NESTING_DEPTH;

fn deep_path(segments: usize) -> HjsonObjectPath {
    let path = (0..segments)
        .map(|i| format!("k{i}"))
        .collect::<Vec<_>>()
        .join(".");
    HjsonObjectPath::parse(&path).unwrap()
}

fn nested_document(depth: usize) -> String {
    let mut text = String::new();
    for i in 0..depth {
        text.push_str(&format!("k{i}: {{\n"));
    }
    text.push_str("leaf: 1\n");
    for _ in 0..depth {
        text.push_str("}\n");
    }
    text
}

// ------------------------------------------------------------------
// set: 編集結果が上限を超えないこと
// ------------------------------------------------------------------

#[test]
fn set_at_depth_limit_is_accepted() {
    let doc = HjsonDocument::parse("k0: {}\n").unwrap();
    // k0 が既に存在するので、結果の深さはセグメント数と一致する
    let edited = doc
        .set(deep_path(MAX_NESTING_DEPTH), 1)
        .expect("上限ちょうどは許可される");
    assert!(HjsonDocument::parse(&edited.to_string()).is_ok());
}

#[test]
fn set_beyond_depth_limit_is_rejected() {
    let doc = HjsonDocument::parse("k0: {}\n").unwrap();
    let result = doc.set(deep_path(MAX_NESTING_DEPTH + 1), 1);
    assert!(matches!(
        result,
        Err(hjson_edit::HjsonEditError::NestingTooDeep { limit, .. }) if limit == MAX_NESTING_DEPTH
    ));
}

#[test]
fn set_with_very_long_path_is_rejected_without_overflow() {
    let doc = HjsonDocument::parse("k0: {}\n").unwrap();
    let result = doc.set(deep_path(20_000), 1);
    assert!(matches!(
        result,
        Err(hjson_edit::HjsonEditError::NestingTooDeep { .. })
    ));
}

#[test]
fn set_replacing_root_with_deep_value_is_rejected() {
    let doc = HjsonDocument::parse("{a: 1}\n").unwrap();
    let mut value = hjson_edit::HjsonValue::Integer(1);
    for i in 0..(MAX_NESTING_DEPTH + 1) {
        value = hjson_edit::HjsonValue::Object(vec![(format!("k{i}"), value)]);
    }

    let result = doc.set(HjsonObjectPath::parse("").unwrap(), value);
    assert!(matches!(
        result,
        Err(hjson_edit::HjsonEditError::NestingTooDeep { .. })
    ));
}

#[test]
fn accepted_set_results_are_always_reparsable() {
    let doc = HjsonDocument::parse("k0: {}\n").unwrap();

    for segments in 1..=MAX_NESTING_DEPTH + 2 {
        if let Ok(edited) = doc.clone().set(deep_path(segments), 1) {
            let text = edited.to_string();
            assert!(
                HjsonDocument::parse(&text).is_ok(),
                "segments={segments} の結果が再パースできない"
            );
        }
    }
}

// ------------------------------------------------------------------
// merge: 編集結果が上限を超えないこと
// ------------------------------------------------------------------

#[test]
fn merge_at_depth_limit_is_accepted() {
    let host = HjsonDocument::parse("k0: {}\n").unwrap();
    let source = HjsonDocument::parse("{a: 1}\n").unwrap();

    // merge はパス末尾もマージ先のオブジェクトになるため、set より 1 段少ない
    let merged = host
        .merge_from(&deep_path(MAX_NESTING_DEPTH - 1), &source)
        .expect("上限ちょうどは許可される");
    assert!(HjsonDocument::parse(&merged.to_string()).is_ok());
}

#[test]
fn merge_beyond_depth_limit_is_rejected() {
    let host = HjsonDocument::parse("k0: {}\n").unwrap();
    let source = HjsonDocument::parse("{a: 1}\n").unwrap();

    let result = host.merge_from(&deep_path(MAX_NESTING_DEPTH), &source);
    assert!(matches!(
        result,
        Err(hjson_edit::HjsonEditError::NestingTooDeep { .. })
    ));
}

#[test]
fn merge_with_deep_source_into_deep_path_is_rejected() {
    let host = HjsonDocument::parse("k0: {}\n").unwrap();
    let source = HjsonDocument::parse(&nested_document(MAX_NESTING_DEPTH - 1)).unwrap();

    let result = host.merge_from(&deep_path(MAX_NESTING_DEPTH - 1), &source);
    assert!(matches!(
        result,
        Err(hjson_edit::HjsonEditError::NestingTooDeep { .. })
    ));
}

#[test]
fn accepted_merge_results_are_always_reparsable() {
    let host = HjsonDocument::parse("k0: {}\n").unwrap();
    let source = HjsonDocument::parse("{a: 1}\n").unwrap();

    for segments in 1..=MAX_NESTING_DEPTH + 2 {
        if let Ok(merged) = host.clone().merge_from(&deep_path(segments), &source) {
            let text = merged.to_string();
            assert!(
                HjsonDocument::parse(&text).is_ok(),
                "segments={segments} の結果が再パースできない"
            );
        }
    }
}

// ------------------------------------------------------------------
// エラー表示が過大な入力で増幅しないこと
// ------------------------------------------------------------------

#[test]
fn path_not_found_error_truncates_long_path() {
    let doc = HjsonDocument::parse("a: 1\n").unwrap();
    let err = doc
        .get(deep_path(1_000))
        .expect_err("存在しないパス")
        .to_string();

    assert!(err.len() < 400, "エラー表示が長すぎる: {} bytes", err.len());
    assert!(err.contains("segments"), "件数が示される: {err}");
}

#[test]
fn invalid_path_error_truncates_long_input() {
    let long = format!("{}.", "k".repeat(5_000));
    let err = HjsonObjectPath::parse(&long)
        .expect_err("不正なパス")
        .to_string();

    assert!(err.len() < 400, "エラー表示が長すぎる: {} bytes", err.len());
    assert!(err.contains("chars"), "文字数が示される: {err}");
}

// ------------------------------------------------------------------
// 1 行のブレースレス文書でも深さ上限を守ること
// ------------------------------------------------------------------

fn single_line_nested(depth: usize) -> String {
    format!("x: {}{}\n", "[".repeat(depth), "]".repeat(depth))
}

#[test]
fn single_line_braceless_document_at_the_limit_is_accepted() {
    // ブレースレス文書の本文も 1 段として数える
    let text = single_line_nested(MAX_NESTING_DEPTH - 1);

    assert!(HjsonDocument::parse(&text).is_ok());
}

#[test]
fn single_line_braceless_document_beyond_the_limit_is_rejected() {
    for depth in [MAX_NESTING_DEPTH, 200] {
        let text = single_line_nested(depth);
        let error = HjsonDocument::parse(&text)
            .expect_err("深さ超過は引用符なし文字列として受理しない")
            .to_string();

        assert!(error.contains("Nesting too deep"), "depth {depth}: {error}");
    }
}
