//! 中間コンテナを作るときの配列インデックスの検証。

use hjson_edit::high_level::error::HjsonEditError;
use hjson_edit::{HjsonDocument, HjsonDocumentEdit, HjsonObjectPath, HjsonValue};

fn path(path: &str) -> HjsonObjectPath {
    HjsonObjectPath::parse(path).unwrap()
}

#[test]
fn creating_an_array_at_a_position_other_than_zero_is_an_error() {
    for target in ["a[5].b", "a[+3].b", "a[1].b", "a[2]"] {
        let error = HjsonDocument::parse("{}")
            .unwrap()
            .set(path(target), HjsonValue::Integer(1))
            .unwrap_err();

        assert!(
            matches!(error, HjsonEditError::ArrayIndexOutOfBounds { .. }),
            "target: {target}, error: {error:?}"
        );
    }
}

#[test]
fn creating_an_array_at_the_first_position_is_allowed() {
    for target in ["a[0].b", "a[+0].b", "a[-].b"] {
        let updated = HjsonDocument::parse("{}")
            .unwrap()
            .set(path(target), HjsonValue::Integer(1))
            .unwrap();

        assert_eq!(
            updated.get(path("a[0].b")).unwrap(),
            HjsonValue::Integer(1),
            "target: {target}"
        );
    }
}

#[test]
fn an_existing_array_still_reports_the_real_length() {
    let document = HjsonDocument::parse("{\n  a: [\n    1\n  ]\n}\n").unwrap();
    let error = document
        .set(path("a[+5]"), HjsonValue::Integer(2))
        .unwrap_err();

    assert!(
        matches!(
            error,
            HjsonEditError::ArrayIndexOutOfBounds { index: 5, len: 1 }
        ),
        "unexpected error: {error:?}"
    );
}

#[test]
fn creating_nested_containers_reports_the_original_error() {
    // 途中のキーが無い場合でも、位置指定の誤りは握りつぶさない
    let error = HjsonDocument::parse("{\n  root: {\n  }\n}\n")
        .unwrap()
        .set(path("root.new[2].value"), HjsonValue::Integer(1))
        .unwrap_err();

    assert!(
        matches!(
            error,
            HjsonEditError::ArrayIndexOutOfBounds { index: 2, .. }
        ),
        "unexpected error: {error:?}"
    );
}
