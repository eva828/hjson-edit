use clap::{Arg, ArgAction, Command};

mod edit;
mod format;
mod parse;

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
        .version("0.1.0")
        .about("HJSON editor preserving comments and formatting")
        .subcommand_required(true)
        .subcommand(parse_command())
        .subcommand(format_command())
        .subcommand(edit_command())
}

fn parse_command() -> Command {
    Command::new("parse")
        .about("Parse HJSON file and output AST")
        .arg(
            Arg::new("file")
                .required(true)
                .value_parser(clap::value_parser!(std::path::PathBuf)),
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
                .value_parser(clap::value_parser!(std::path::PathBuf)),
        )
}
