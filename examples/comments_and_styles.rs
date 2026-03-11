use hjson_edit::{CommentPath, CommentStyle, StringStyle, HjsonDocument};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let input = r#"{
  name: my-app
  version: 1.0.0
}"#;

    let doc = HjsonDocument::parse(input)
        .map_err(|e| format!("Parse errors: {:?}", e))?;

    // 1. Add a header comment to the document
    let doc = doc.comment_set(
        &CommentPath::Header,
        " This file was generated automatically.\n Do not edit manually.",
        Some(CommentStyle::Line), // Generates `// ...`
    )?;

    // 2. Add an inline comment to a specific key
    let doc = doc.comment_set(
        &CommentPath::Inline { dot_path: "name".to_string() },
        " The name of your application",
        Some(CommentStyle::Hash), // Generates `# ...`
    )?;
    
    // 3. Add a comment before a specific key
    let doc = doc.comment_set(
        &CommentPath::Key { dot_path: "version".to_string() },
        " Use Semantic Versioning",
        Some(CommentStyle::Block), // Generates `/* ... */`
    )?;

    // 4. Set string values with specific formatting styles
    // Multi-line block string (using `'''`)
    let doc = doc.set_with_style(
        "description",
        "A multi-line\ndescription goes here.",
        StringStyle::Block,
    )?;

    // Unquoted string
    let doc = doc.set_with_style(
        "type",
        "library",
        StringStyle::Unquoted,
    )?;

    println!("=== Styled & Commented Document ===");
    println!("{}", doc);

    Ok(())
}
