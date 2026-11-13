//! 新しいメンバーの体裁は先頭メンバーから写すが、コメントと改行は写さない。

use hjson_edit::high_level::ops::object::HjsonDocumentEdit;
use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::{HjsonDocument, HjsonValue};

fn set(src: &str, path: &str) -> HjsonDocument {
    HjsonDocument::parse(src)
        .unwrap()
        .set(
            HjsonObjectPath::parse(path).unwrap(),
            HjsonValue::Integer(7),
        )
        .unwrap_or_else(|error| panic!("{src:?} +{path}: {error}"))
}

#[test]
fn the_comment_after_the_colon_is_not_copied_to_the_new_member() {
    for (src, path, expected, comments) in [
        (
            "{a: /* one */ 1}\n",
            "b",
            "{a: /* one */ 1, b: 7}\n",
            ["one"],
        ),
        (
            "{a: /* one */ 1, b: 2}\n",
            "c",
            "{a: /* one */ 1, b: 2, c: 7}\n",
            ["one"],
        ),
        (
            "x: {a: /* c */ 1}\n",
            "x.b",
            "x: {a: /* c */ 1, b: 7}\n",
            ["c"],
        ),
        ("{a: 1 /* t */}\n", "b", "{a: 1 /* t */, b: 7}\n", ["t"]),
    ] {
        let updated = set(src, path);

        assert_eq!(updated.to_string(), expected, "{src:?}");
        assert_eq!(updated.all_comments(), comments, "{src:?}");
    }
}

#[test]
fn the_spacing_after_the_colon_is_kept() {
    assert_eq!(set("{a:  1}\n", "b").to_string(), "{a:  1, b:  7}\n");
    assert_eq!(set("{a:1}\n", "b").to_string(), "{a:1, b:7}\n");
    assert_eq!(set("{a: 1}\n", "b").to_string(), "{a: 1, b: 7}\n");
}

#[test]
fn a_newline_after_the_colon_is_not_copied() {
    assert_eq!(set("{a:\n  1}\n", "b").to_string(), "{a:\n  1, b: 7}\n");
}
