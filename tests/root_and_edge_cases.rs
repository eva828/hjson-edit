use hjson_edit::{HjsonDocument, HjsonDocumentEdit, HjsonObjectPath};

#[test]
fn parse_completely_empty_file_roundtrips() {
    let src = "";
    let doc = HjsonDocument::parse(src).unwrap();
    assert_eq!(doc.to_string(), src);
}

#[test]
fn set_in_empty_document_inserts_braceless_object_member() {
    let doc = HjsonDocument::parse("").unwrap();
    let new_doc = doc.set(HjsonObjectPath::parse("foo").unwrap(), 1).unwrap();
    assert_eq!(new_doc.to_string(), "foo: 1");
}

#[test]
fn parse_whitespace_only_file_roundtrips() {
    let src = "   \n  \t  \n";
    let doc = HjsonDocument::parse(src).unwrap();
    assert_eq!(doc.to_string(), src);
}

#[test]
fn set_in_whitespace_only_document_preserves_whitespace_and_adds_member() {
    let doc = HjsonDocument::parse("   \n  \t  \n").unwrap();
    let new_doc = doc.set(HjsonObjectPath::parse("foo").unwrap(), 1).unwrap();
    assert_eq!(new_doc.to_string(), "   \n  \t  \nfoo: 1");
}

#[test]
fn parse_comment_only_file_roundtrips() {
    let src = "// just a comment\n/* block */\n# hash comment";
    let doc = HjsonDocument::parse(src).unwrap();
    assert_eq!(doc.to_string(), src);
}

#[test]
fn set_in_comment_only_document_preserves_comments_and_adds_member() {
    let doc = HjsonDocument::parse("// just a comment\n/* block */\n# hash comment").unwrap();
    let new_doc = doc.set(HjsonObjectPath::parse("foo").unwrap(), 1).unwrap();
    assert!(new_doc.to_string().contains("foo: 1"));
    assert!(new_doc.to_string().contains("just a comment"));
}

#[test]
fn parse_single_scalar_value_roundtrips() {
    let src = "1";
    let doc = HjsonDocument::parse(src).unwrap();
    assert_eq!(doc.to_string(), src);
}

#[test]
fn set_on_scalar_root_returns_error() {
    let doc = HjsonDocument::parse("1").unwrap();
    assert!(doc.set(HjsonObjectPath::parse("foo").unwrap(), 1).is_err());
}
