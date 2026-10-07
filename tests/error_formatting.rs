use hjson_edit::high_level::{
    ops::object::HjsonDocumentObjectEdit, path::HjsonObjectPath, HjsonDocument,
};

#[test]
fn parse_error_includes_line_and_column() {
    let result = HjsonDocument::parse("{\n  \"a\": 1");
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("line") || msg.contains("column") || msg.contains("offset"),
        "error message should contain line/column info, got: {}",
        msg
    );
}

#[test]
fn path_not_found_error_includes_path() {
    let doc = HjsonDocument::parse("{\n  name: app\n}\n").unwrap();
    let result = doc.get(HjsonObjectPath::parse("missing.path").unwrap());
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("missing") || msg.contains("path"),
        "error message should contain 'missing' or 'path', got: {}",
        msg
    );
}
