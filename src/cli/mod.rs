use clap::{Arg, ArgAction, Command};

use anyhow::{Context, Result};
use hjson_edit::MAX_INPUT_BYTES;
use std::path::Path;

mod edit;
mod format;
mod parse;

/// 標準入力をファイル名として表す引数。
pub(crate) fn is_stdin(path: &Path) -> bool {
    path == Path::new("-")
}

/// 入力を読み込む前にサイズを検査する。
///
/// 上限を超える入力は位置情報を正しく扱えないため、読み込まずにエラーを返す。
/// `-` は標準入力として読む。
pub(crate) fn read_input(path: &Path) -> Result<String> {
    let text = if is_stdin(path) {
        use std::io::Read;

        let mut text = String::new();
        std::io::stdin()
            .lock()
            .take(MAX_INPUT_BYTES as u64 + 1)
            .read_to_string(&mut text)
            .context("Failed to read standard input")?;
        if text.len() > MAX_INPUT_BYTES {
            anyhow::bail!("Input too long: limit is {MAX_INPUT_BYTES} bytes");
        }
        text
    } else {
        let metadata = std::fs::metadata(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;
        let len = metadata.len();
        if len > MAX_INPUT_BYTES as u64 {
            anyhow::bail!("Input too long: {len} bytes, limit is {MAX_INPUT_BYTES} bytes");
        }

        std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?
    };

    // BOM は読み込みの層で除去する（パーサーは通常の文字として扱う）。
    Ok(text
        .strip_prefix('\u{feff}')
        .map_or(text.clone(), str::to_string))
}

/// `--in-place` を標準入力と一緒に使えないことを確かめる。
pub(crate) fn reject_in_place_stdin(matches: &clap::ArgMatches, path: &Path) -> Result<()> {
    if matches.get_flag("in-place") && is_stdin(path) {
        anyhow::bail!("--in-place cannot be used with standard input");
    }

    Ok(())
}

/// ファイルへ書き込む。
///
/// 同じディレクトリの一時ファイルへ書いてから rename する。書き込みに失敗しても
/// 元のファイルを壊さない（`fs::write` は書き込み前に切り詰めるため）。
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<()> {
    use std::io::Write;

    // シンボリックリンクは参照先を置き換える（rename はリンク自体を置き換えてしまう）
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let directory = target
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map_or_else(
            || std::path::PathBuf::from("."),
            std::path::Path::to_path_buf,
        );
    let name = target
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid file path: {}", target.display()))?
        .to_string_lossy()
        .into_owned();

    let mut attempt = 0u32;
    let (temp_path, mut file) = loop {
        let candidate = directory.join(format!(".{name}.{}.{attempt}.tmp", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (candidate, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && attempt < 100 => {
                attempt += 1;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to write file: {}", target.display()))
            }
        }
    };

    let written = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all());
    drop(file);

    let result = written.and_then(|()| {
        if let Ok(metadata) = std::fs::metadata(&target) {
            std::fs::set_permissions(&temp_path, metadata.permissions())?;
        }
        std::fs::rename(&temp_path, &target)
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }

    result.with_context(|| format!("Failed to write file: {}", target.display()))
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
                .help("HJSON file to read (use - for standard input)"),
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
                .help("HJSON file to read (use - for standard input)"),
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
        .arg(
            Arg::new("no-verify")
                .long("no-verify")
                .action(ArgAction::SetTrue)
                .help("Skip validating that the result keeps the values and comments"),
        )
        .arg(
            Arg::new("in-place")
                .long("in-place")
                .action(ArgAction::SetTrue)
                .conflicts_with("output")
                .help("Write the result back to the input file"),
        )
        .arg(
            Arg::new("output")
                .long("output")
                .value_name("output")
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .help("Write the result to this file instead of stdout"),
        )
}
