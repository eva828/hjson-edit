use hjson_edit::{HjsonDocument, HjsonDocumentObjectEdit, HjsonValue};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let input = r#"{
  name: my-app
  version: 1.0.0
}"#;

    let doc = HjsonDocument::parse(input).map_err(|e| format!("Parse errors: {:?}", e))?;

    // Modify the document using the available methods
    let doc = doc
        .set("description", "A multi-line\ndescription goes here.")?
        .set("type", "library")?
        .set(
            "keywords",
            vec![
                HjsonValue::String("hjson".to_string()),
                HjsonValue::String("parser".to_string()),
            ],
        )?;

    println!("=== Modified Document ===");
    println!("{}", doc.to_string());

    Ok(())
}
