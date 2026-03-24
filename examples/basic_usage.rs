use hjson_edit::{HjsonDocument, HjsonDocumentObjectEdit};
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
    match doc.get("dependencies.clap") {
        Ok(val) => println!("clap version: {:?}", val),
        Err(e) => println!("Error getting value: {:?}", e),
    }

    // Modify the document - each operation returns a Result
    let updated = doc
        .set("version", "0.1.0")? // Adds a new key
        .set("dependencies.clap", "4.6")? // Updates an existing key
        .set("features[-]", "extra")? // Adds to an array using push syntax
        .remove("name")?; // Removes a key

    println!("\n=== Modified ===");
    println!("{}", updated.to_string());

    Ok(())
}
