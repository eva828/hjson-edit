use hjson_edit::high_level::path::HjsonObjectPath;

// ------------------------------------------------------------------
// Edge cases for path parsing.
// ------------------------------------------------------------------

#[test]
fn parse_leading_dot_skips_empty_key() {
    // ".a" -> leading dot is skipped, resulting in single key "a"
    let path = HjsonObjectPath::parse(".a").unwrap();
    assert_eq!(path.depth(), 1);
}

#[test]
fn parse_trailing_dot_skips_empty_key() {
    // "a." -> trailing dot is skipped, resulting in single key "a"
    let path = HjsonObjectPath::parse("a.").unwrap();
    assert_eq!(path.depth(), 1);
}

#[test]
fn parse_multiple_consecutive_dots_skips_empty() {
    // "a...b" -> keys "a" and "b" (empty keys between ... are skipped)
    let path = HjsonObjectPath::parse("a...b").unwrap();
    assert_eq!(path.depth(), 2);
}

#[test]
fn parse_unicode_segment_succeeds() {
    let path = HjsonObjectPath::parse("日本語").unwrap();
    assert_eq!(path.depth(), 1);
}

#[test]
fn parse_unicode_mixed_path_succeeds() {
    let path = HjsonObjectPath::parse("日本語.English").unwrap();
    assert_eq!(path.depth(), 2);
}
