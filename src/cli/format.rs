use anyhow::{Context, Result};
use std::path::PathBuf;

use hjson_edit::HjsonDocument;
use hjson_edit::{FormatConfig, Formatter};

pub fn run_format(matches: &clap::ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    match HjsonDocument::parse(&content) {
        Ok(document) => {
            let node = document.into_green_node();
            let config = FormatConfig::default();
            let formatted = Formatter::format_with_depth(node, 0, config);
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
