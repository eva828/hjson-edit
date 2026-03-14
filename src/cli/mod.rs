use clap::{Arg, ArgAction, ArgMatches, Command};
use std::path::PathBuf;

use anyhow::{Context, Result};
use hjson_edit::{CommentPath, CommentStyle, HjsonDocument, HjsonValue, StringStyle};

#[derive(Debug, Clone)]
enum EditOp {
    ObjectSet {
        path: String,
        value: HjsonValue,
        string_style: Option<StringStyle>,
    },
    ObjectRemove {
        path: String,
    },
    ArrayAppend {
        path: String,
        value: HjsonValue,
        string_style: Option<StringStyle>,
    },
    ArrayInsert {
        path: String,
        index: usize,
        value: HjsonValue,
        string_style: Option<StringStyle>,
    },
    ArrayRemove {
        path: String,
        index: usize,
    },
    CommentSet {
        path: CommentPath,
        text: String,
        style: Option<CommentStyle>,
    },
    CommentAppend {
        path: CommentPath,
        text: String,
        style: Option<CommentStyle>,
    },
    CommentRemove {
        path: CommentPath,
    },
    SetCommentStyle(CommentStyle),
    SetStringStyle(StringStyle),
}

struct CliApp;

impl CliApp {
    fn build() -> Command {
        Command::new("hjson-edit")
            .version("0.1.0")
            .about("HJSON editor preserving comments and formatting")
            .subcommand_required(true)
            .subcommand(Self::parse_command())
            .subcommand(Self::format_command())
            .subcommand(Self::edit_command())
    }

    fn parse_command() -> Command {
        Command::new("parse")
            .about("Parse HJSON file and output AST")
            .arg(
                Arg::new("file")
                    .required(true)
                    .value_parser(clap::value_parser!(PathBuf)),
            )
    }

    fn format_command() -> Command {
        Command::new("format")
            .about("Format the entire HJSON document")
            .arg(
                Arg::new("file")
                    .required(true)
                    .value_parser(clap::value_parser!(PathBuf)),
            )
    }

    fn edit_command() -> Command {
        Command::new("edit")
            .about("Edit HJSON, then format only changed containers")
            .arg(
                Arg::new("file")
                    .required(true)
                    .value_parser(clap::value_parser!(PathBuf)),
            )
            .arg(
                Arg::new("object-set")
                    .long("object-set")
                    .aliases(["set"])
                    .action(ArgAction::Append)
                    .num_args(2)
                    .value_names(["PATH", "VALUE"]),
            )
            .arg(
                Arg::new("object-remove")
                    .long("object-remove")
                    .aliases(["remove"])
                    .action(ArgAction::Append)
                    .value_name("PATH"),
            )
            .arg(
                Arg::new("array-append")
                    .long("array-append")
                    .action(ArgAction::Append)
                    .num_args(2)
                    .value_names(["PATH", "VALUE"]),
            )
            .arg(
                Arg::new("array-insert")
                    .long("array-insert")
                    .action(ArgAction::Append)
                    .num_args(3)
                    .value_names(["PATH", "INDEX", "VALUE"]),
            )
            .arg(
                Arg::new("array-remove")
                    .long("array-remove")
                    .action(ArgAction::Append)
                    .num_args(2)
                    .value_names(["PATH", "INDEX"]),
            )
            .arg(
                Arg::new("comment-set")
                    .long("comment-set")
                    .action(ArgAction::Append)
                    .num_args(2)
                    .value_names(["PATH", "TEXT"]),
            )
            .arg(
                Arg::new("comment-append")
                    .long("comment-append")
                    .action(ArgAction::Append)
                    .num_args(2)
                    .value_names(["PATH", "TEXT"]),
            )
            .arg(
                Arg::new("comment-remove")
                    .long("comment-remove")
                    .action(ArgAction::Append)
                    .value_name("PATH"),
            )
            .arg(
                Arg::new("comment-style")
                    .long("comment-style")
                    .action(ArgAction::Append)
                    .value_name("STYLE")
                    .value_parser(["line", "hash", "block"]),
            )
            .arg(
                Arg::new("string-style")
                    .long("string-style")
                    .action(ArgAction::Append)
                    .value_name("STYLE")
                    .value_parser(["preserve", "auto", "double", "block", "unquoted"]),
            )
            .arg(
                Arg::new("in-place")
                    .long("in-place")
                    .conflicts_with("output")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("output")
                    .long("output")
                    .conflicts_with("in-place")
                    .value_parser(clap::value_parser!(PathBuf)),
            )
    }
}

struct CliValueParser;

impl CliValueParser {
    fn parse_set_arg(path: &str, value_str: &str) -> Result<(String, HjsonValue)> {
        if let Some(colon_pos) = path.rfind(':') {
            let hint = &path[colon_pos + 1..];
            let base_path = path[..colon_pos].to_string();

            let value = match hint {
                "int" => value_str
                    .parse::<i64>()
                    .map(HjsonValue::Integer)
                    .map_err(|_| anyhow::anyhow!("Invalid integer: {}", value_str))?,
                "float" => value_str
                    .parse::<f64>()
                    .map(HjsonValue::Float)
                    .map_err(|_| anyhow::anyhow!("Invalid float: {}", value_str))?,
                "bool" => match value_str {
                    "true" => HjsonValue::Bool(true),
                    "false" => HjsonValue::Bool(false),
                    _ => anyhow::bail!("Invalid bool: {}", value_str),
                },
                "str" => HjsonValue::String(value_str.to_string()),
                "null" => HjsonValue::Null,
                _ => return Ok((path.to_string(), Self::parse_value(value_str))),
            };

            return Ok((base_path, value));
        }

        Ok((path.to_string(), Self::parse_value(value_str)))
    }

    fn parse_value(raw: &str) -> HjsonValue {
        match raw {
            "null" => return HjsonValue::Null,
            "true" => return HjsonValue::Bool(true),
            "false" => return HjsonValue::Bool(false),
            _ => {}
        }

        // Try to parse as number
        if let Ok(n) = raw.parse::<i64>() {
            return HjsonValue::Integer(n);
        }
        if let Ok(n) = raw.parse::<f64>() {
            return HjsonValue::Float(n);
        }

        if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
            return HjsonValue::String(raw[1..raw.len() - 1].to_string());
        }

        if raw.len() >= 6 && raw.starts_with("'''") && raw.ends_with("'''") {
            return HjsonValue::String(raw[3..raw.len() - 3].to_string());
        }

        HjsonValue::String(raw.to_string())
    }

    fn detect_inline_string_style(raw: &str) -> Option<StringStyle> {
        if raw.len() >= 6 && raw.starts_with("'''") && raw.ends_with("'''") {
            return Some(StringStyle::Block);
        }
        if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
            return Some(StringStyle::DoubleQuoted);
        }
        None
    }

    fn parse_comment_path(raw: &str) -> CommentPath {
        if raw == ":header" {
            CommentPath::Header
        } else if raw == ":footer" {
            CommentPath::Footer
        } else if let Some(stripped) = raw.strip_suffix(":inline") {
            CommentPath::Inline {
                dot_path: stripped.to_string(),
            }
        } else {
            CommentPath::Key {
                dot_path: raw.to_string(),
            }
        }
    }
}

struct EditOpCollector<'a> {
    matches: &'a ArgMatches,
    ops_with_indices: Vec<(usize, EditOp)>,
}

impl<'a> EditOpCollector<'a> {
    fn new(matches: &'a ArgMatches) -> Self {
        Self {
            matches,
            ops_with_indices: Vec::new(),
        }
    }

    fn collect(mut self) -> Result<Vec<EditOp>> {
        self.collect_object_ops()?;
        self.collect_array_ops()?;
        self.collect_comment_ops()?;
        self.collect_style_ops()?;
        self.ops_with_indices.sort_by_key(|(idx, _)| *idx);
        Ok(self
            .ops_with_indices
            .into_iter()
            .map(|(_, op)| op)
            .collect())
    }

    fn collect_object_ops(&mut self) -> Result<()> {
        if let Some(sets) = self.matches.get_raw("object-set") {
            let vals: Vec<_> = sets.collect();
            let indices = self.collect_indices("object-set");

            for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
                if chunk.len() != 2 {
                    continue;
                }
                let path = chunk[0].to_string_lossy().to_string();
                let value_str = chunk[1].to_string_lossy().to_string();
                let (parsed_path, value) = CliValueParser::parse_set_arg(&path, &value_str)?;
                self.ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::ObjectSet {
                        path: parsed_path,
                        value,
                        string_style: CliValueParser::detect_inline_string_style(&value_str),
                    },
                ));
            }
        }

        if let Some(removes) = self.matches.get_raw("object-remove") {
            let indices = self.collect_indices("object-remove");
            for (idx, val) in indices.iter().zip(removes) {
                self.ops_with_indices.push((
                    *idx,
                    EditOp::ObjectRemove {
                        path: val.to_string_lossy().to_string(),
                    },
                ));
            }
        }

        Ok(())
    }

    fn collect_array_ops(&mut self) -> Result<()> {
        if let Some(appends) = self.matches.get_raw("array-append") {
            let vals: Vec<_> = appends.collect();
            let indices = self.collect_indices("array-append");

            for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
                if chunk.len() != 2 {
                    continue;
                }
                let path = chunk[0].to_string_lossy().to_string();
                let raw_value = chunk[1].to_string_lossy();
                self.ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::ArrayAppend {
                        path,
                        value: CliValueParser::parse_value(&raw_value),
                        string_style: CliValueParser::detect_inline_string_style(&raw_value),
                    },
                ));
            }
        }

        if let Some(inserts) = self.matches.get_raw("array-insert") {
            let vals: Vec<_> = inserts.collect();
            let indices = self.collect_indices("array-insert");

            for (chunk, idx_chunk) in vals.chunks(3).zip(indices.chunks(3)) {
                if chunk.len() != 3 {
                    continue;
                }
                let path = chunk[0].to_string_lossy().to_string();
                let index = Self::parse_index(&chunk[1].to_string_lossy())?;
                let raw_value = chunk[2].to_string_lossy();

                self.ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::ArrayInsert {
                        path,
                        index,
                        value: CliValueParser::parse_value(&raw_value),
                        string_style: CliValueParser::detect_inline_string_style(&raw_value),
                    },
                ));
            }
        }

        if let Some(removes) = self.matches.get_raw("array-remove") {
            let vals: Vec<_> = removes.collect();
            let indices = self.collect_indices("array-remove");

            for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
                if chunk.len() != 2 {
                    continue;
                }
                let path = chunk[0].to_string_lossy().to_string();
                let index = Self::parse_index(&chunk[1].to_string_lossy())?;
                self.ops_with_indices
                    .push((idx_chunk[0], EditOp::ArrayRemove { path, index }));
            }
        }

        Ok(())
    }

    fn collect_comment_ops(&mut self) -> Result<()> {
        self.collect_comment_set_ops();
        self.collect_comment_append_ops();
        self.collect_comment_remove_ops();
        Ok(())
    }

    fn collect_comment_set_ops(&mut self) {
        if let Some(sets) = self.matches.get_raw("comment-set") {
            let vals: Vec<_> = sets.collect();
            let indices = self.collect_indices("comment-set");
            for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
                if chunk.len() != 2 {
                    continue;
                }
                self.ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::CommentSet {
                        path: CliValueParser::parse_comment_path(&chunk[0].to_string_lossy()),
                        text: chunk[1].to_string_lossy().to_string(),
                        style: None,
                    },
                ));
            }
        }
    }

    fn collect_comment_append_ops(&mut self) {
        if let Some(appends) = self.matches.get_raw("comment-append") {
            let vals: Vec<_> = appends.collect();
            let indices = self.collect_indices("comment-append");
            for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
                if chunk.len() != 2 {
                    continue;
                }
                self.ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::CommentAppend {
                        path: CliValueParser::parse_comment_path(&chunk[0].to_string_lossy()),
                        text: chunk[1].to_string_lossy().to_string(),
                        style: None,
                    },
                ));
            }
        }
    }

    fn collect_comment_remove_ops(&mut self) {
        if let Some(removes) = self.matches.get_raw("comment-remove") {
            let indices = self.collect_indices("comment-remove");
            for (idx, val) in indices.iter().zip(removes) {
                self.ops_with_indices.push((
                    *idx,
                    EditOp::CommentRemove {
                        path: CliValueParser::parse_comment_path(&val.to_string_lossy()),
                    },
                ));
            }
        }
    }

    fn collect_style_ops(&mut self) -> Result<()> {
        self.collect_comment_style_ops()?;
        self.collect_string_style_ops()?;
        Ok(())
    }

    fn collect_comment_style_ops(&mut self) -> Result<()> {
        if let Some(styles) = self.matches.get_raw("comment-style") {
            let indices = self.collect_indices("comment-style");
            for (idx, val) in indices.iter().zip(styles) {
                let style = match val.to_string_lossy().as_ref() {
                    "line" => CommentStyle::Line,
                    "hash" => CommentStyle::Hash,
                    "block" => CommentStyle::Block,
                    other => anyhow::bail!("Invalid comment style: {}", other),
                };
                self.ops_with_indices
                    .push((*idx, EditOp::SetCommentStyle(style)));
            }
        }
        Ok(())
    }

    fn collect_string_style_ops(&mut self) -> Result<()> {
        if let Some(styles) = self.matches.get_raw("string-style") {
            let indices = self.collect_indices("string-style");
            for (idx, val) in indices.iter().zip(styles) {
                let style = match val.to_string_lossy().as_ref() {
                    "preserve" => StringStyle::Preserve,
                    "auto" => StringStyle::Auto,
                    "double" => StringStyle::DoubleQuoted,
                    "block" => StringStyle::Block,
                    "unquoted" => StringStyle::Unquoted,
                    other => anyhow::bail!("Invalid string style: {}", other),
                };
                self.ops_with_indices
                    .push((*idx, EditOp::SetStringStyle(style)));
            }
        }
        Ok(())
    }

    fn collect_indices(&self, name: &str) -> Vec<usize> {
        self.matches.indices_of(name).unwrap_or_default().collect()
    }

    fn parse_index(raw: &str) -> Result<usize> {
        raw.parse::<usize>()
            .map_err(|_| anyhow::anyhow!("Invalid array index: {}", raw))
    }
}

struct ParseReporter;

impl ParseReporter {
    fn render(errors: &[hjson_edit::ParseError]) -> String {
        let mut message = String::from("Parse errors:");
        for err in errors {
            message.push_str(&format!(
                "\n  Line {}, Column {}: {}",
                err.line, err.column, err.message
            ));
        }
        message
    }
}

pub fn run() -> Result<()> {
    let matches = build_cli().get_matches();

    match matches.subcommand() {
        Some(("parse", sub_matches)) => run_parse(sub_matches),
        Some(("format", sub_matches)) => run_format(sub_matches),
        Some(("edit", sub_matches)) => run_edit(sub_matches),
        _ => anyhow::bail!("Usage: hjson-edit <command> [args]\nCommands: parse, format, edit"),
    }
}

pub fn build_cli() -> Command {
    CliApp::build()
}

fn run_parse(matches: &ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    match HjsonDocument::parse(&content) {
        Ok(_) => {
            println!("Parsed successfully!");
            Ok(())
        }
        Err(errors) => anyhow::bail!(ParseReporter::render(&errors)),
    }
}

fn run_format(matches: &ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    match HjsonDocument::parse(&content) {
        Ok(document) => {
            let formatted = document.format_document()?;
            print!("{}", formatted);
            Ok(())
        }
        Err(errors) => anyhow::bail!(ParseReporter::render(&errors)),
    }
}

fn run_edit(matches: &ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let mut doc = match HjsonDocument::parse(&content) {
        Ok(document) => document,
        Err(errors) => anyhow::bail!(ParseReporter::render(&errors)),
    };

    let ops = collect_ops(matches)?;
    let mut current_comment_style: Option<CommentStyle> = None;
    let mut current_string_style: Option<StringStyle> = None;

    for op in ops {
        match op {
            EditOp::SetCommentStyle(style) => current_comment_style = Some(style),
            EditOp::SetStringStyle(style) => current_string_style = Some(style),
            EditOp::ObjectSet {
                path,
                value,
                string_style,
            } => {
                doc = doc.set_with_style(
                    path.as_str(),
                    value,
                    string_style
                        .or(current_string_style)
                        .unwrap_or(StringStyle::Preserve),
                )?;
            }
            EditOp::ObjectRemove { path } => doc = doc.remove(path.as_str())?,
            EditOp::ArrayAppend {
                path,
                value,
                string_style,
            } => {
                doc = doc.push_with_style(
                    path.as_str(),
                    value,
                    string_style
                        .or(current_string_style)
                        .unwrap_or(StringStyle::Auto),
                )?;
            }
            EditOp::ArrayInsert {
                path,
                index,
                value,
                string_style,
            } => {
                doc = doc.insert_at_with_style(
                    path.as_str(),
                    index,
                    value,
                    string_style
                        .or(current_string_style)
                        .unwrap_or(StringStyle::Auto),
                )?;
            }
            EditOp::ArrayRemove { path, index } => doc = doc.remove_at(path.as_str(), index)?,
            EditOp::CommentSet { path, text, style } => {
                doc = doc.comment_set(&path, &text, style.or(current_comment_style))?;
            }
            EditOp::CommentAppend { path, text, style } => {
                doc = doc.comment_append(&path, &text, style.or(current_comment_style))?;
            }
            EditOp::CommentRemove { path } => doc = doc.comment_remove(&path)?,
        }
    }

    let result = doc.to_string();
    if matches.get_flag("in-place") {
        std::fs::write(path, &result)
            .with_context(|| format!("Failed to write file: {}", path.display()))?;
    } else if let Some(output_path) = matches.get_one::<PathBuf>("output") {
        std::fs::write(output_path, &result)
            .with_context(|| format!("Failed to write file: {}", output_path.display()))?;
    } else {
        print!("{}", result);
    }

    Ok(())
}

fn collect_ops(matches: &ArgMatches) -> Result<Vec<EditOp>> {
    EditOpCollector::new(matches).collect()
}
