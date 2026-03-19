use anyhow::{Context, Result};
use std::path::PathBuf;

use hjson_edit::{FormatConfig, Formatter};
use hjson_edit::{HjsonDocument, LineIndex};

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
            let line_index = LineIndex::new(&content);
            let message = format!(
                "Parse errors:\n  {}",
                errors
                    .iter()
                    .map(|err| err.format(&line_index))
                    .collect::<Vec<String>>()
                    .join("\n  ")
            );
            message
        }),
    }
}
