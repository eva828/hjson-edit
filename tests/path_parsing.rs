use std::convert::TryFrom;

use hjson_edit::{
    high_level::{
        error::HjsonEditError,
        path::{HjsonCommentPath, HjsonObjectPath},
    },
};

#[test]
fn parse_valid_simple_key_succeeds() {
    let path = HjsonObjectPath::parse("name").unwrap();
    assert_eq!(path.depth(), 1);
}

#[test]
fn parse_valid_nested_key_succeeds() {
    let path = HjsonObjectPath::parse("server.host").unwrap();
    assert_eq!(path.depth(), 2);
}

#[test]
fn parse_valid_array_index_succeeds() {
    let path = HjsonObjectPath::parse("items[0]").unwrap();
    assert_eq!(path.depth(), 2);
}

#[test]
fn parse_valid_array_push_succeeds() {
    let path = HjsonObjectPath::parse("items[-]").unwrap();
    assert_eq!(path.depth(), 2);
}

#[test]
fn parse_valid_array_insert_at_succeeds() {
    let path = HjsonObjectPath::parse("items[+1]").unwrap();
    assert_eq!(path.depth(), 2);
}

#[test]
fn parse_valid_complex_mixed_path_succeeds() {
    let path = HjsonObjectPath::parse("a.b[0].c[1][2]").unwrap();
    assert_eq!(path.depth(), 6);
}

#[test]
fn parse_empty_path_succeeds() {
    let path = HjsonObjectPath::parse("").unwrap();
    assert_eq!(path.depth(), 0);
}

#[test]
fn parse_unclosed_bracket_fails() {
    let result = HjsonObjectPath::parse("name[");
    assert!(result.is_err());
}

#[test]
fn parse_non_numeric_index_fails() {
    let result = HjsonObjectPath::parse("name[abc]");
    assert!(result.is_err());
}

#[test]
fn parse_plus_without_number_fails() {
    let result = HjsonObjectPath::parse("name[+]");
    assert!(result.is_err());
}

#[test]
fn parse_double_dot_skips_empty_key() {
    // "name..nested" -> "name" and "nested"; empty key between .. is skipped
    let path = HjsonObjectPath::parse("name..nested").unwrap();
    assert_eq!(path.depth(), 2);
}

#[test]
fn parse_comment_path_header_succeeds() {
    let path = HjsonCommentPath::try_from(":header").unwrap();
    assert!(matches!(path, HjsonCommentPath::Header));
}

#[test]
fn parse_comment_path_footer_succeeds() {
    let path = HjsonCommentPath::try_from(":footer").unwrap();
    assert!(matches!(path, HjsonCommentPath::Footer));
}

#[test]
fn parse_comment_path_before_succeeds() {
    let path = HjsonCommentPath::try_from("name:before").unwrap();
    assert!(matches!(path, HjsonCommentPath::Before(_)));
}

#[test]
fn parse_comment_path_inline_succeeds() {
    let path = HjsonCommentPath::try_from("name:inline").unwrap();
    assert!(matches!(path, HjsonCommentPath::Inline(_)));
}

#[test]
fn parse_comment_path_nested_before_succeeds() {
    let path = HjsonCommentPath::try_from("a.b:before").unwrap();
    assert!(matches!(path, HjsonCommentPath::Before(_)));
}

#[test]
fn parse_comment_path_unknown_suffix_defaults_to_before() {
    // "name:unknown" does not match :inline/:header/:footer, so falls through to Before
    let path = HjsonCommentPath::try_from("name:unknown").unwrap();
    assert!(matches!(path, HjsonCommentPath::Before(_)));
}

#[test]
fn path_split_first_returns_first_and_rest() {
    let path = HjsonObjectPath::parse("a.b.c").unwrap();
    let (first, rest) = path.split_first().unwrap();
    assert_eq!(rest.depth(), 2);
}

#[test]
fn path_split_last_returns_rest_and_last() {
    let path = HjsonObjectPath::parse("a.b.c").unwrap();
    let (rest, last) = path.split_last().unwrap();
    assert_eq!(rest.depth(), 2);
}

#[test]
fn path_split_at_returns_two_halves() {
    let path = HjsonObjectPath::parse("a.b.c").unwrap();
    let (left, right) = path.split_at(1).unwrap();
    assert_eq!(left.depth(), 1);
    assert_eq!(right.depth(), 2);
}
