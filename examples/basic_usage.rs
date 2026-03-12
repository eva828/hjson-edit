use hjson_edit::HjsonDocument;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Parse an HJSON string
    let input = r#"{
  name: example
  dependencies: {
    clap: 4.5
  }
  features: [
    default
  ]
}"#;

    let doc = HjsonDocument::parse(input).map_err(|e| format!("Parse errors: {:?}", e))?;

    // Read values using dot notation or array indexing
    if let Some(val) = doc.get("dependencies.clap") {
        println!("clap version: {:?}", val);
    }

    // Modify the document
    let updated = doc
        .set("version", "0.1.0")? // Adds a new key
        .set("dependencies.clap", "4.6")? // Updates an existing key
        .push("features", "extra")? // Adds to an array
        .remove("name")?; // Removes a key

    println!("\n=== Modified ===");
    println!("{}", updated);

    // Get changed lines for external diffing/patching (1-indexed)
    println!("\nChanged lines: {:?}", doc.changed_lines(&updated));

    Ok(())
}
