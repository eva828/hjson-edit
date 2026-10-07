use hjson_edit::{HjsonDocument, HjsonDocumentObjectEdit, HjsonObjectPath};
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
    match doc.get(HjsonObjectPath::parse("dependencies.clap").unwrap()) {
        Ok(val) => println!("clap version: {:?}", val),
        Err(e) => println!("Error getting value: {:?}", e),
    }

    // Modify the document - each operation returns a Result
    let updated = doc
        .set(HjsonObjectPath::parse("version").unwrap(), "0.1.0")? // Adds a new key
        .set(HjsonObjectPath::parse("dependencies.clap").unwrap(), "4.6")? // Updates an existing key
        .set(HjsonObjectPath::parse("features[-]").unwrap(), "extra")? // Adds to an array using push syntax
        .remove(HjsonObjectPath::parse("name").unwrap())?; // Removes a key

    println!("\n=== Modified ===");
    println!("{}", updated);

    Ok(())
}
