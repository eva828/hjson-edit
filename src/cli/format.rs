use anyhow::{Context, Result};
use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::high_level::HjsonDocument;
use hjson_edit::IndentStyle;
use std::path::PathBuf;

pub fn run_format(matches: &clap::ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = super::read_input(path)?;

    let doc = match HjsonDocument::parse(&content) {
        Ok(document) => {
            super::warn_skipped_input(&document);
            document
        }
        Err(error) => anyhow::bail!("{error}"),
    };

    // 既定は文書から推定し、指定された項目だけ上書きする
    let detected = doc.detected_format_config();
    let mut config = detected.clone();
    let detected_width = match detected.indent() {
        IndentStyle::Spaces(width) => width,
        IndentStyle::Tabs => 2,
    };
    match matches
        .get_one::<String>("indent-style")
        .map(String::as_str)
    {
        Some("tabs") => config = config.with_indent(IndentStyle::Tabs),
        Some("spaces") => {
            let width = matches
                .get_one::<u8>("indent-width")
                .copied()
                .unwrap_or(detected_width);
            config = config.with_indent(IndentStyle::Spaces(width));
        }
        None => {
            if let Some(width) = matches.get_one::<u8>("indent-width").copied() {
                config = config.with_indent(IndentStyle::Spaces(width));
            }
        }
        Some(_) => {}
    }
    if let Some(newline) = matches.get_one::<String>("newline") {
        config = config.with_newline(if newline == "crlf" { "\r\n" } else { "\n" });
    }

    let doc = doc.verify_edits(!matches.get_flag("no-verify"));

    // 整形は値とコメントを変えてはならない（format_container が検証する）
    let empty = HjsonObjectPath::parse("").expect("the empty path is always valid");
    let formatted = doc
        .format_container(&empty, &config)
        .context("Failed to format the document")?;
    let mut result = formatted.to_string();

    // 出力は改行で終える（末尾に改行の無い入力でも 1 行として扱えるようにする）
    if !result.is_empty() && !result.ends_with('\n') {
        result.push_str(config.newline());
    }

    print!("{}", result);
    Ok(())
}
