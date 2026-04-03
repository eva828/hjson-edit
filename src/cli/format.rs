use anyhow::{Context, Result};
use hjson_edit::high_level::HjsonDocument;
use std::path::PathBuf;

use hjson_edit::{FormatConfig, Formatter};

pub fn run_format(matches: &clap::ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    match HjsonDocument::parse(&content) {
        Ok(document) => {
            let config = FormatConfig::default();
            let ast = document.ast()?;
            let formatted = Formatter::format(&ast, config);
            print!("{}", formatted);
            Ok(())
        }
        Err(error) => anyhow::bail!("Parse errors:\n  {}", error),
    }
}
