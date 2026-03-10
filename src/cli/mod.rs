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
    Command::new("hjson-edit")
        .version("0.1.0")
        .about("HJSON editor preserving comments and formatting")
        .subcommand_required(true)
        .subcommand(
            Command::new("parse")
                .about("Parse HJSON file and output AST")
                .arg(
                    Arg::new("file")
                        .required(true)
                        .value_parser(clap::value_parser!(PathBuf)),
                ),
        )
        .subcommand(
            Command::new("format")
                .about("Format the entire HJSON document")
                .arg(
                    Arg::new("file")
                        .required(true)
                        .value_parser(clap::value_parser!(PathBuf)),
                ),
        )
        .subcommand(
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
                ),
        )
}

fn run_parse(matches: &ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    match hjson_edit::parse(&content) {
        Ok(result) => {
            let red = hjson_edit::RedNode::new_root(result.green);
            println!("Parsed successfully!");
            println!("AST: {:?}", red);
            Ok(())
        }
        Err(errors) => {
            let mut message = String::from("Parse errors:");
            for err in errors {
                message.push_str(&format!(
                    "\n  Line {}, Column {}: {}",
                    err.line, err.column, err.message
                ));
            }
            anyhow::bail!(message)
        }
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
        Err(errors) => {
            let mut message = String::from("Parse errors:");
            for err in errors {
                message.push_str(&format!(
                    "\n  Line {}, Column {}: {}",
                    err.line, err.column, err.message
                ));
            }
            anyhow::bail!(message)
        }
    }
}

fn run_edit(matches: &ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let mut doc = match HjsonDocument::parse(&content) {
        Ok(document) => document,
        Err(errors) => {
            let mut message = String::from("Parse errors:");
            for err in errors {
                message.push_str(&format!(
                    "\n  Line {}, Column {}: {}",
                    err.line, err.column, err.message
                ));
            }
            anyhow::bail!(message)
        }
    };

    let ops = collect_ops(matches)?;
    let mut current_comment_style: Option<CommentStyle> = None;
    let mut current_string_style: Option<StringStyle> = None;

    for op in ops {
        match op {
            EditOp::SetCommentStyle(style) => {
                current_comment_style = Some(style);
            }
            EditOp::SetStringStyle(style) => {
                current_string_style = Some(style);
            }
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
            EditOp::ObjectRemove { path } => {
                doc = doc.remove(path.as_str())?;
            }
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
            EditOp::ArrayRemove { path, index } => {
                doc = doc.remove_at(path.as_str(), index)?;
            }
            EditOp::CommentSet { path, text, style } => {
                doc = doc.comment_set(&path, &text, style.or(current_comment_style))?;
            }
            EditOp::CommentAppend { path, text, style } => {
                doc = doc.comment_append(&path, &text, style.or(current_comment_style))?;
            }
            EditOp::CommentRemove { path } => {
                doc = doc.comment_remove(&path)?;
            }
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
    let mut ops_with_indices: Vec<(usize, EditOp)> = Vec::new();

    if let Some(sets) = matches.get_raw("object-set") {
        let vals: Vec<_> = sets.collect();
        let indices: Vec<_> = matches
            .indices_of("object-set")
            .unwrap_or_default()
            .collect();

        for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
            if chunk.len() == 2 {
                let path = chunk[0].to_string_lossy().to_string();
                let value_str = chunk[1].to_string_lossy().to_string();
                let (parsed_path, value) = parse_set_arg(&path, &value_str)?;
                ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::ObjectSet {
                        path: parsed_path,
                        value,
                        string_style: detect_inline_string_style(&value_str),
                    },
                ));
            }
        }
    }

    if let Some(removes) = matches.get_raw("object-remove") {
        let indices: Vec<_> = matches
            .indices_of("object-remove")
            .unwrap_or_default()
            .collect();
        for (idx, val) in indices.iter().zip(removes) {
            let path = val.to_string_lossy().to_string();
            ops_with_indices.push((*idx, EditOp::ObjectRemove { path }));
        }
    }

    if let Some(appends) = matches.get_raw("array-append") {
        let vals: Vec<_> = appends.collect();
        let indices: Vec<_> = matches
            .indices_of("array-append")
            .unwrap_or_default()
            .collect();

        for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
            if chunk.len() == 2 {
                let path = chunk[0].to_string_lossy().to_string();
                let raw_value = chunk[1].to_string_lossy();
                ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::ArrayAppend {
                        path,
                        value: parse_value(&raw_value),
                        string_style: detect_inline_string_style(&raw_value),
                    },
                ));
            }
        }
    }

    if let Some(inserts) = matches.get_raw("array-insert") {
        let vals: Vec<_> = inserts.collect();
        let indices: Vec<_> = matches
            .indices_of("array-insert")
            .unwrap_or_default()
            .collect();

        for (chunk, idx_chunk) in vals.chunks(3).zip(indices.chunks(3)) {
            if chunk.len() == 3 {
                let path = chunk[0].to_string_lossy().to_string();
                let index: usize = chunk[1].to_string_lossy().parse().map_err(|_| {
                    anyhow::anyhow!("Invalid array index: {}", chunk[1].to_string_lossy())
                })?;
                let raw_value = chunk[2].to_string_lossy();

                ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::ArrayInsert {
                        path,
                        index,
                        value: parse_value(&raw_value),
                        string_style: detect_inline_string_style(&raw_value),
                    },
                ));
            }
        }
    }

    if let Some(removes) = matches.get_raw("array-remove") {
        let vals: Vec<_> = removes.collect();
        let indices: Vec<_> = matches
            .indices_of("array-remove")
            .unwrap_or_default()
            .collect();

        for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
            if chunk.len() == 2 {
                let path = chunk[0].to_string_lossy().to_string();
                let index: usize = chunk[1].to_string_lossy().parse().map_err(|_| {
                    anyhow::anyhow!("Invalid array index: {}", chunk[1].to_string_lossy())
                })?;
                ops_with_indices.push((idx_chunk[0], EditOp::ArrayRemove { path, index }));
            }
        }
    }

    if let Some(sets) = matches.get_raw("comment-set") {
        let vals: Vec<_> = sets.collect();
        let indices: Vec<_> = matches
            .indices_of("comment-set")
            .unwrap_or_default()
            .collect();

        for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
            if chunk.len() == 2 {
                let path_str = chunk[0].to_string_lossy().to_string();
                let text = chunk[1].to_string_lossy().to_string();
                ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::CommentSet {
                        path: parse_comment_path(&path_str),
                        text,
                        style: None,
                    },
                ));
            }
        }
    }

    if let Some(appends) = matches.get_raw("comment-append") {
        let vals: Vec<_> = appends.collect();
        let indices: Vec<_> = matches
            .indices_of("comment-append")
            .unwrap_or_default()
            .collect();

        for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
            if chunk.len() == 2 {
                let path_str = chunk[0].to_string_lossy().to_string();
                let text = chunk[1].to_string_lossy().to_string();
                ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::CommentAppend {
                        path: parse_comment_path(&path_str),
                        text,
                        style: None,
                    },
                ));
            }
        }
    }

    if let Some(removes) = matches.get_raw("comment-remove") {
        let indices: Vec<_> = matches
            .indices_of("comment-remove")
            .unwrap_or_default()
            .collect();

        for (idx, val) in indices.iter().zip(removes) {
            let path_str = val.to_string_lossy().to_string();
            ops_with_indices.push((
                *idx,
                EditOp::CommentRemove {
                    path: parse_comment_path(&path_str),
                },
            ));
        }
    }

    if let Some(styles) = matches.get_raw("comment-style") {
        let indices: Vec<_> = matches
            .indices_of("comment-style")
            .unwrap_or_default()
            .collect();

        for (idx, val) in indices.iter().zip(styles) {
            let style = match val.to_string_lossy().as_ref() {
                "line" => CommentStyle::Line,
                "hash" => CommentStyle::Hash,
                "block" => CommentStyle::Block,
                other => anyhow::bail!("Invalid comment style: {}", other),
            };
            ops_with_indices.push((*idx, EditOp::SetCommentStyle(style)));
        }
    }

    if let Some(styles) = matches.get_raw("string-style") {
        let indices: Vec<_> = matches
            .indices_of("string-style")
            .unwrap_or_default()
            .collect();

        for (idx, val) in indices.iter().zip(styles) {
            let style = match val.to_string_lossy().as_ref() {
                "preserve" => StringStyle::Preserve,
                "auto" => StringStyle::Auto,
                "double" => StringStyle::DoubleQuoted,
                "block" => StringStyle::Block,
                "unquoted" => StringStyle::Unquoted,
                other => anyhow::bail!("Invalid string style: {}", other),
            };
            ops_with_indices.push((*idx, EditOp::SetStringStyle(style)));
        }
    }

    ops_with_indices.sort_by_key(|(idx, _)| *idx);
    Ok(ops_with_indices.into_iter().map(|(_, op)| op).collect())
}

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
            _ => return Ok((path.to_string(), parse_value(value_str))),
        };

        return Ok((base_path, value));
    }

    Ok((path.to_string(), parse_value(value_str)))
}

fn parse_value(s: &str) -> HjsonValue {
    if s == "null" {
        return HjsonValue::Null;
    }
    if s == "true" {
        return HjsonValue::Bool(true);
    }
    if s == "false" {
        return HjsonValue::Bool(false);
    }

    if is_hjson_integer(s) {
        let v = s.parse::<i64>().expect("validated integer");
        return HjsonValue::Integer(v);
    }

    if is_hjson_float(s) {
        let v = s.parse::<f64>().expect("validated float");
        return HjsonValue::Float(v);
    }

    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        return HjsonValue::String(s[1..s.len() - 1].to_string());
    }

    if s.len() >= 6 && s.starts_with("'''") && s.ends_with("'''") {
        return HjsonValue::String(s[3..s.len() - 3].to_string());
    }

    HjsonValue::String(s.to_string())
}

fn detect_inline_string_style(s: &str) -> Option<StringStyle> {
    if s.len() >= 6 && s.starts_with("'''") && s.ends_with("'''") {
        return Some(StringStyle::Block);
    }
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        return Some(StringStyle::DoubleQuoted);
    }
    None
}

fn is_hjson_integer(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let bytes = s.as_bytes();
    let mut idx = 0usize;

    if bytes[idx] == b'-' {
        idx += 1;
        if idx == bytes.len() {
            return false;
        }
    }

    if bytes[idx] == b'0' {
        idx + 1 == bytes.len()
    } else if bytes[idx].is_ascii_digit() {
        idx += 1;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        idx == bytes.len()
    } else {
        false
    }
}

fn is_hjson_float(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let bytes = s.as_bytes();
    let mut idx = 0usize;

    if bytes[idx] == b'-' {
        idx += 1;
        if idx == bytes.len() {
            return false;
        }
    }

    if bytes[idx] == b'0' {
        idx += 1;
        if idx < bytes.len() && bytes[idx].is_ascii_digit() {
            return false;
        }
    } else if bytes[idx].is_ascii_digit() {
        idx += 1;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
    } else {
        return false;
    }

    let mut has_fraction_or_exp = false;

    if idx < bytes.len() && bytes[idx] == b'.' {
        has_fraction_or_exp = true;
        idx += 1;
        let frac_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if frac_start == idx {
            return false;
        }
    }

    if idx < bytes.len() && matches!(bytes[idx], b'e' | b'E') {
        has_fraction_or_exp = true;
        idx += 1;
        if idx < bytes.len() && matches!(bytes[idx], b'+' | b'-') {
            idx += 1;
        }
        let exp_start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if exp_start == idx {
            return false;
        }
    }

    has_fraction_or_exp && idx == bytes.len()
}

fn parse_comment_path(s: &str) -> CommentPath {
    if s == ":header" {
        CommentPath::Header
    } else if s == ":footer" {
        CommentPath::Footer
    } else if let Some(stripped) = s.strip_suffix(":inline") {
        CommentPath::Inline {
            dot_path: stripped.to_string(),
        }
    } else {
        CommentPath::Key {
            dot_path: s.to_string(),
        }
    }
}
