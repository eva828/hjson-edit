use hjson_edit::ast::CommentKind;

#[test]
fn decode_line_comment_strips_prefix_and_leading_space() {
    assert_eq!(CommentKind::Line.decode_body("// comment"), "comment");
}

#[test]
fn decode_line_comment_strips_prefix_without_space() {
    assert_eq!(CommentKind::Line.decode_body("//comment"), "comment");
}

#[test]
fn decode_line_comment_strips_multiple_leading_spaces() {
    assert_eq!(CommentKind::Line.decode_body("//  comment"), "comment");
}

#[test]
fn decode_hash_comment_strips_prefix_and_leading_space() {
    assert_eq!(CommentKind::Hash.decode_body("# comment"), "comment");
}

#[test]
fn decode_hash_comment_strips_prefix_without_space() {
    assert_eq!(CommentKind::Hash.decode_body("#comment"), "comment");
}

#[test]
fn decode_block_comment_strips_delimiters_and_trims() {
    assert_eq!(CommentKind::Block.decode_body("/* comment */"), "comment");
}

#[test]
fn decode_block_comment_strips_delimiters_with_extra_whitespace() {
    assert_eq!(CommentKind::Block.decode_body("/*  comment  */"), "comment");
}
