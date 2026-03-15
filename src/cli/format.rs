use anyhow::{Context, Result};
use std::path::PathBuf;

use hjson_edit::HjsonDocument;

pub fn run_format(matches: &clap::ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    match HjsonDocument::parse(&content) {
        Ok(document) => {
            let formatted = document.format_document()?;
            print!("{}", formatted);
            Ok(())
        }
        Err(errors) => anyhow::bail!({
            let mut message = String::from("Parse errors:");
            for err in errors {
                message.push_str(&format!(
                    "\n  Line {}, Column {}: {}",
                    err.line, err.column, err.message
                ));
            }
            message
        }),
    }
}
