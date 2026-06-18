use anyhow::{Context, Result};
use hjson_edit::high_level::HjsonDocument;
use hjson_edit::{Format, FormatConfig, IndentStyle};
use hjson_edit::RedNode;
use std::path::PathBuf;

pub fn run_format(matches: &clap::ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let doc = match HjsonDocument::parse(&content) {
        Ok(document) => document,
        Err(error) => anyhow::bail!("Parse errors:\n  {}", error),
    };

    let indent = match matches.get_one::<String>("indent-style").map(String::as_str) {
        Some("tabs") => IndentStyle::Tabs,
        _ => IndentStyle::Spaces(*matches.get_one::<u8>("indent-width").unwrap_or(&2)),
    };
    let newline = match matches.get_one::<String>("newline").map(String::as_str) {
        Some("crlf") => "\r\n",
        _ => "\n",
    };

    let config = FormatConfig::default()
        .with_indent(indent)
        .with_newline(newline);

    let ast = doc.ast().context("Failed to build semantic AST")?;
    let formatted = ast.format(&config);
    let result = RedNode::new_root(formatted).text().to_string();

    print!("{}", result);
    Ok(())
}
