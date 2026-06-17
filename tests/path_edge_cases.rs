use hjson_edit::high_level::path::HjsonObjectPath;

#[test]
fn parse_leading_dot_fails() {
    let result = HjsonObjectPath::parse(".a");
    assert!(result.is_err());
}


#[test]
fn parse_trailing_dot_fails() {
    let result = HjsonObjectPath::parse("a.");
    assert!(result.is_err());
}


#[test]
fn parse_multiple_consecutive_dots_fails() {
    let result = HjsonObjectPath::parse("a...b");
    assert!(result.is_err());
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
