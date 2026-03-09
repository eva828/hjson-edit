//! HJSON Editor CLI

use clap::{Arg, ArgAction, Command};
use std::path::PathBuf;

use hjson_edit::{CommentPath, CommentStyle, HjsonDocument, HjsonValue, StringStyle};

// =============================================================================
// EditOp - Internal representation of edit operations
// =============================================================================

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

// =============================================================================
// Main
// =============================================================================

fn main() {
    let cli = build_cli();
    let matches = cli.get_matches();

    match matches.subcommand() {
        Some(("parse", sub_matches)) => {
            run_parse(sub_matches);
        }
        Some(("format", sub_matches)) => {
            run_format(sub_matches);
        }
        Some(("edit", sub_matches)) => {
            run_edit(sub_matches);
        }
        _ => {
            eprintln!("Usage: hjson-edit <command> [args]");
            eprintln!("Commands: parse, format, edit");
            std::process::exit(1);
        }
    }
}

// =============================================================================
// CLI Builder
// =============================================================================

fn build_cli() -> Command {
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
                .about("Parse and re-output (format preservation test)")
                .arg(
                    Arg::new("file")
                        .required(true)
                        .value_parser(clap::value_parser!(PathBuf)),
                ),
        )
        .subcommand(
            Command::new("edit")
                .about("Edit HJSON file with various operations")
                .arg(
                    Arg::new("file")
                        .required(true)
                        .value_parser(clap::value_parser!(PathBuf)),
                )
                // Object operations
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
                // Array operations
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
                // Comment operations
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
                        .value_parser([
                            "preserve", "auto", "double", "single", "block", "unquoted",
                        ]),
                )
                // Output options
                .arg(
                    Arg::new("in-place")
                        .long("in-place")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("output")
                        .long("output")
                        .value_parser(clap::value_parser!(PathBuf)),
                ),
        )
}

// =============================================================================
// Subcommand Handlers
// =============================================================================

fn run_parse(matches: &clap::ArgMatches) {
    let path: &PathBuf = matches.get_one("file").unwrap();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read file: {}", e);
            std::process::exit(1);
        }
    };

    match hjson_edit::parse(&content) {
        Ok(result) => {
            let red = hjson_edit::RedNode::new_root(result.green);
            println!("Parsed successfully!");
            println!("AST: {:?}", red);
        }
        Err(errors) => {
            eprintln!("Parse errors:");
            for err in errors {
                eprintln!(
                    "  Line {}, Column {}: {}",
                    err.line, err.column, err.message
                );
            }
            std::process::exit(1);
        }
    }
}

fn run_format(matches: &clap::ArgMatches) {
    let path: &PathBuf = matches.get_one("file").unwrap();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read file: {}", e);
            std::process::exit(1);
        }
    };

    match hjson_edit::parse(&content) {
        Ok(result) => {
            let output = hjson_edit::green_to_string(&result.green);
            print!("{}", output);
        }
        Err(errors) => {
            eprintln!("Parse errors:");
            for err in errors {
                eprintln!(
                    "  Line {}, Column {}: {}",
                    err.line, err.column, err.message
                );
            }
            std::process::exit(1);
        }
    }
}

fn run_edit(matches: &clap::ArgMatches) {
    let path: &PathBuf = matches.get_one("file").unwrap();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read file: {}", e);
            std::process::exit(1);
        }
    };

    let doc = match HjsonDocument::parse(&content) {
        Ok(d) => d,
        Err(errors) => {
            eprintln!("Parse errors:");
            for err in errors {
                eprintln!(
                    "  Line {}, Column {}: {}",
                    err.line, err.column, err.message
                );
            }
            std::process::exit(1);
        }
    };

    // Collect operations with their indices for ordering
    let ops = collect_ops(matches);

    // Apply operations sequentially
    let mut doc = doc;
    let mut current_style: Option<CommentStyle> = None;
    let mut current_string_style: Option<StringStyle> = None;

    for op in ops {
        match op {
            EditOp::SetCommentStyle(style) => {
                current_style = Some(style);
            }
            EditOp::SetStringStyle(style) => {
                current_string_style = Some(style);
            }
            EditOp::ObjectSet {
                path,
                value,
                string_style,
            } => {
                doc = match doc.set_with_style(
                    path.as_str(),
                    value,
                    string_style
                        .or(current_string_style)
                        .unwrap_or(StringStyle::Preserve),
                ) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error setting {}: {}", path, e);
                        std::process::exit(1);
                    }
                };
            }
            EditOp::ObjectRemove { path } => {
                doc = match doc.remove(path.as_str()) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error removing {}: {}", path, e);
                        std::process::exit(1);
                    }
                };
            }
            EditOp::ArrayAppend {
                path,
                value,
                string_style,
            } => {
                doc = match doc.push_with_style(
                    path.as_str(),
                    value,
                    string_style
                        .or(current_string_style)
                        .unwrap_or(StringStyle::Auto),
                ) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error appending to {}: {}", path, e);
                        std::process::exit(1);
                    }
                };
            }
            EditOp::ArrayInsert {
                path,
                index,
                value,
                string_style,
            } => {
                doc = match doc.insert_at_with_style(
                    path.as_str(),
                    index,
                    value,
                    string_style
                        .or(current_string_style)
                        .unwrap_or(StringStyle::Auto),
                ) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error inserting into {} at {}: {}", path, index, e);
                        std::process::exit(1);
                    }
                };
            }
            EditOp::ArrayRemove { path, index } => {
                doc = match doc.remove_at(path.as_str(), index) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error removing from {} at {}: {}", path, index, e);
                        std::process::exit(1);
                    }
                };
            }
            EditOp::CommentSet { path, text, style } => {
                let effective_style = style.or(current_style);
                doc = match doc.comment_set(&path, &text, effective_style) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error setting comment for {:?}: {}", path, e);
                        std::process::exit(1);
                    }
                };
            }
            EditOp::CommentAppend { path, text, style } => {
                let effective_style = style.or(current_style);
                doc = match doc.comment_append(&path, &text, effective_style) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error appending comment for {:?}: {}", path, e);
                        std::process::exit(1);
                    }
                };
            }
            EditOp::CommentRemove { path } => {
                doc = match doc.comment_remove(&path) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error removing comment for {:?}: {}", path, e);
                        std::process::exit(1);
                    }
                };
            }
        }
    }

    // Output
    let result = doc.to_string();
    if matches.get_flag("in-place") {
        if let Err(e) = std::fs::write(path, &result) {
            eprintln!("Failed to write file: {}", e);
            std::process::exit(1);
        }
    } else if let Some(output_path) = matches.get_one::<PathBuf>("output") {
        if let Err(e) = std::fs::write(output_path, &result) {
            eprintln!("Failed to write file: {}", e);
            std::process::exit(1);
        }
    } else {
        print!("{}", result);
    }
}

// =============================================================================
// Operation Collection and Ordering
// =============================================================================

fn collect_ops(matches: &clap::ArgMatches) -> Vec<EditOp> {
    let mut ops_with_indices: Vec<(usize, EditOp)> = Vec::new();

    // Object set operations - get_raw returns flat list, group by num_args(2)
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
                match parse_set_arg(&path, &value_str) {
                    Ok((parsed_path, value)) => {
                        ops_with_indices.push((
                            idx_chunk[0],
                            EditOp::ObjectSet {
                                path: parsed_path,
                                value,
                                string_style: detect_inline_string_style(&value_str),
                            },
                        ));
                    }
                    Err(e) => {
                        eprintln!("Invalid set argument '{}': {}", path, e);
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    // Object remove operations
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

    // Array append operations - get_raw returns flat list, group by num_args(2)
    if let Some(appends) = matches.get_raw("array-append") {
        let vals: Vec<_> = appends.collect();
        let indices: Vec<_> = matches
            .indices_of("array-append")
            .unwrap_or_default()
            .collect();
        for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
            if chunk.len() == 2 {
                let path = chunk[0].to_string_lossy().to_string();
                let value = parse_value(&chunk[1].to_string_lossy());
                ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::ArrayAppend {
                        path,
                        value,
                        string_style: detect_inline_string_style(&chunk[1].to_string_lossy()),
                    },
                ));
            }
        }
    }

    // Array insert operations - get_raw returns flat list, group by num_args(3)
    if let Some(inserts) = matches.get_raw("array-insert") {
        let vals: Vec<_> = inserts.collect();
        let indices: Vec<_> = matches
            .indices_of("array-insert")
            .unwrap_or_default()
            .collect();
        for (chunk, idx_chunk) in vals.chunks(3).zip(indices.chunks(3)) {
            if chunk.len() == 3 {
                let path = chunk[0].to_string_lossy().to_string();
                let index: usize = match chunk[1].to_string_lossy().parse() {
                    Ok(i) => i,
                    Err(_) => {
                        eprintln!("Invalid array index: {}", chunk[1].to_string_lossy());
                        std::process::exit(1);
                    }
                };
                let value = parse_value(&chunk[2].to_string_lossy());
                ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::ArrayInsert {
                        path,
                        index,
                        value,
                        string_style: detect_inline_string_style(&chunk[2].to_string_lossy()),
                    },
                ));
            }
        }
    }

    // Array remove operations - get_raw returns flat list, group by num_args(2)
    if let Some(removes) = matches.get_raw("array-remove") {
        let vals: Vec<_> = removes.collect();
        let indices: Vec<_> = matches
            .indices_of("array-remove")
            .unwrap_or_default()
            .collect();
        for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
            if chunk.len() == 2 {
                let path = chunk[0].to_string_lossy().to_string();
                let index: usize = match chunk[1].to_string_lossy().parse() {
                    Ok(i) => i,
                    Err(_) => {
                        eprintln!("Invalid array index: {}", chunk[1].to_string_lossy());
                        std::process::exit(1);
                    }
                };
                ops_with_indices.push((idx_chunk[0], EditOp::ArrayRemove { path, index }));
            }
        }
    }

    // Comment set operations - get_raw returns flat list, group by num_args(2)
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
                let path = parse_comment_path(&path_str);
                ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::CommentSet {
                        path,
                        text,
                        style: None,
                    },
                ));
            }
        }
    }

    // Comment append operations - get_raw returns flat list, group by num_args(2)
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
                let path = parse_comment_path(&path_str);
                ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::CommentAppend {
                        path,
                        text,
                        style: None,
                    },
                ));
            }
        }
    }

    // Comment remove operations
    if let Some(removes) = matches.get_raw("comment-remove") {
        let indices: Vec<_> = matches
            .indices_of("comment-remove")
            .unwrap_or_default()
            .collect();
        for (idx, val) in indices.iter().zip(removes) {
            let path_str = val.to_string_lossy().to_string();
            let path = parse_comment_path(&path_str);
            ops_with_indices.push((*idx, EditOp::CommentRemove { path }));
        }
    }

    // Comment style operations
    if let Some(styles) = matches.get_raw("comment-style") {
        let indices: Vec<_> = matches
            .indices_of("comment-style")
            .unwrap_or_default()
            .collect();
        for (idx, val) in indices.iter().zip(styles) {
            let style_str = val.to_string_lossy().to_string();
            let style = match style_str.as_str() {
                "line" => CommentStyle::Line,
                "hash" => CommentStyle::Hash,
                "block" => CommentStyle::Block,
                _ => {
                    eprintln!("Invalid comment style: {}", style_str);
                    std::process::exit(1);
                }
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
                "single" => StringStyle::SingleQuoted,
                "block" => StringStyle::Block,
                "unquoted" => StringStyle::Unquoted,
                other => {
                    eprintln!("Invalid string style: {}", other);
                    std::process::exit(1);
                }
            };
            ops_with_indices.push((*idx, EditOp::SetStringStyle(style)));
        }
    }

    // Sort by index to preserve original order
    ops_with_indices.sort_by_key(|(idx, _)| *idx);
    ops_with_indices.into_iter().map(|(_, op)| op).collect()
}

// =============================================================================
// Argument Parsing
// =============================================================================

fn parse_set_arg(path: &str, value_str: &str) -> Result<(String, HjsonValue), String> {
    // Check for type hint: path:type
    if let Some(colon_pos) = path.rfind(':') {
        let hint = &path[colon_pos + 1..];
        let base_path = path[..colon_pos].to_string();

        let value = match hint {
            "int" => value_str
                .parse::<i64>()
                .map(HjsonValue::Integer)
                .map_err(|_| format!("Invalid integer: {}", value_str))?,
            "float" => value_str
                .parse::<f64>()
                .map(HjsonValue::Float)
                .map_err(|_| format!("Invalid float: {}", value_str))?,
            "bool" => match value_str {
                "true" => HjsonValue::Bool(true),
                "false" => HjsonValue::Bool(false),
                _ => return Err(format!("Invalid bool: {}", value_str)),
            },
            "str" => HjsonValue::String(value_str.to_string()),
            "hex" => {
                if value_str.starts_with("0x") || value_str.starts_with("0X") {
                    i64::from_str_radix(&value_str[2..], 16)
                        .map(HjsonValue::HexInteger)
                        .map_err(|_| format!("Invalid hex: {}", value_str))?
                } else {
                    return Err(format!("Hex value must start with 0x: {}", value_str));
                }
            }
            "null" => HjsonValue::Null,
            _ => {
                // Not a type hint, treat as part of path
                return Ok((path.to_string(), parse_value(value_str)));
            }
        };
        return Ok((base_path, value));
    }

    Ok((path.to_string(), parse_value(value_str)))
}

fn parse_value(s: &str) -> HjsonValue {
    // Check for special values first
    if s.eq_ignore_ascii_case("-Infinity") {
        return HjsonValue::NegInfinity;
    }
    if s.eq_ignore_ascii_case("Infinity") {
        return HjsonValue::Infinity;
    }
    if s.eq_ignore_ascii_case("NaN") {
        return HjsonValue::NaN;
    }
    if s.eq_ignore_ascii_case("null") {
        return HjsonValue::Null;
    }
    if s.eq_ignore_ascii_case("true") {
        return HjsonValue::Bool(true);
    }
    if s.eq_ignore_ascii_case("false") {
        return HjsonValue::Bool(false);
    }

    // Check for hex
    if s.starts_with("0x") || s.starts_with("0X") {
        if let Ok(v) = i64::from_str_radix(&s[2..], 16) {
            return HjsonValue::HexInteger(v);
        }
    }

    // Check for integer
    if let Ok(v) = s.parse::<i64>() {
        return HjsonValue::Integer(v);
    }

    // Check for float
    if let Ok(v) = s.parse::<f64>() {
        return HjsonValue::Float(v);
    }

    // Quoted string
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        return HjsonValue::String(s[1..s.len() - 1].to_string());
    }
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        return HjsonValue::String(s[1..s.len() - 1].to_string());
    }
    if s.len() >= 6 && s.starts_with("'''") && s.ends_with("'''") {
        return HjsonValue::String(s[3..s.len() - 3].to_string());
    }

    // Default: unquoted string
    HjsonValue::String(s.to_string())
}

fn detect_inline_string_style(s: &str) -> Option<StringStyle> {
    if s.len() >= 6 && s.starts_with("'''") && s.ends_with("'''") {
        return Some(StringStyle::Block);
    }
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        return Some(StringStyle::DoubleQuoted);
    }
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        return Some(StringStyle::SingleQuoted);
    }
    None
}

fn parse_comment_path(s: &str) -> CommentPath {
    if s == ":header" {
        CommentPath::Header
    } else if s == ":footer" {
        CommentPath::Footer
    } else if let Some(stripped) = s.strip_suffix(":inline") {
        let dot_path = stripped.to_string();
        CommentPath::Inline { dot_path }
    } else {
        CommentPath::Key {
            dot_path: s.to_string(),
        }
    }
}
