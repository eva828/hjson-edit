use clap::{Arg, ArgAction, Command};

use anyhow::{Context, Result};
use hjson_edit::MAX_INPUT_BYTES;
use std::path::Path;

mod edit;
mod format;
mod parse;

/// ファイルを読み込む前にサイズを検査する。
///
/// 上限を超える入力は位置情報を正しく扱えないため、読み込まずにエラーを返す。
pub(crate) fn read_input(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    let len = metadata.len();
    if len > MAX_INPUT_BYTES as u64 {
        anyhow::bail!("Input too long: {len} bytes, limit is {MAX_INPUT_BYTES} bytes");
    }

    let text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    // BOM はファイル読み込みの層で除去する（パーサーは通常の文字として扱う）。
    Ok(text
        .strip_prefix('\u{feff}')
        .map_or(text.clone(), str::to_string))
}

/// パース時に読み飛ばした入力を警告として stderr に出す。
pub(crate) fn warn_skipped_input(document: &hjson_edit::HjsonDocument) {
    for warning in document.warnings() {
        eprintln!("Warning: skipped input: {warning}");
    }
}

pub use edit::edit_command;
pub use format::run_format;
pub use parse::run_parse;

pub fn run() -> anyhow::Result<()> {
    let matches = build_cli().get_matches();

    match matches.subcommand() {
        Some(("parse", sub_matches)) => run_parse(sub_matches),
        Some(("format", sub_matches)) => run_format(sub_matches),
        Some(("edit", sub_matches)) => edit::run_edit(sub_matches),
        _ => anyhow::bail!("Usage: hjson-edit <command> [args]\nCommands: parse, format, edit"),
    }
}

pub fn build_cli() -> Command {
    Command::new("hjson-edit")
        .version(env!("CARGO_PKG_VERSION"))
        .about("HJSON editor preserving comments and formatting")
        .subcommand_required(true)
        .subcommand(parse_command())
        .subcommand(format_command())
        .subcommand(edit_command())
}

fn parse_command() -> Command {
    Command::new("parse")
        .about("Parse HJSON file and output tree or AST")
        .arg(
            Arg::new("file")
                .required(true)
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .help("HJSON file to read"),
        )
        .arg(
            Arg::new("ast")
                .long("ast")
                .action(ArgAction::SetTrue)
                .help("Show the semantic AST tree instead of the lossless tree view"),
        )
        .arg(
            Arg::new("show-trivia")
                .long("show-trivia")
                .action(ArgAction::SetTrue)
                .help("Show trivia tokens (whitespace, newlines, comments)"),
        )
        .arg(
            Arg::new("color")
                .long("color")
                .value_parser(["always", "auto", "never"])
                .default_value("auto")
                .help("Color output: always, auto (default), never"),
        )
        .arg(
            Arg::new("simple")
                .long("simple")
                .action(ArgAction::SetTrue)
                .help("Simple output without tree lines and colors"),
        )
}

fn format_command() -> Command {
    Command::new("format")
        .about("Format the entire HJSON document")
        .arg(
            Arg::new("file")
                .required(true)
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .help("HJSON file to read"),
        )
        .arg(
            Arg::new("indent-style")
                .long("indent-style")
                .value_parser(["spaces", "tabs"])
                .help("Indent style: spaces or tabs (default: detected from the document)"),
        )
        .arg(
            Arg::new("indent-width")
                .long("indent-width")
                .value_parser(clap::value_parser!(u8).range(1..=8))
                .help("Indent width in spaces (default: detected from the document)"),
        )
        .arg(
            Arg::new("newline")
                .long("newline")
                .value_parser(["lf", "crlf"])
                .help("Newline style: lf or crlf (default: detected from the document)"),
        )
}
