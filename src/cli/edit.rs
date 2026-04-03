use clap::{Arg, ArgAction, ArgMatches, Command};

use anyhow::{Context, Result};
use hjson_edit::high_level::{
    ops::{
        comment::{CommentStyle, HjsonDocumentCommentEdit},
        object::{HjsonDocumentObjectEdit, StringStyle},
    },
    path::HjsonCommentPath,
    value::HjsonValue,
    HjsonDocument,
};

#[derive(Debug, Clone)]
enum EditOp {
    Set {
        path: String,
        value: HjsonValue,
        string_style: Option<StringStyle>,
    },
    Remove {
        path: String,
    },
    CommentSet {
        path: HjsonCommentPath,
        text: String,
    },
    CommentAppend {
        path: HjsonCommentPath,
        text: String,
    },
    CommentRemove {
        path: HjsonCommentPath,
    },
    SetCommentStyle(CommentStyle),
    SetStringStyle(StringStyle),
}

pub fn edit_command() -> Command {
    Command::new("edit")
        .about("Edit HJSON, then format only changed containers")
        .arg(
            Arg::new("file")
                .required(true)
                .value_parser(clap::value_parser!(std::path::PathBuf)),
        )
        .arg(
            Arg::new("set")
                .long("set")
                .short('s')
                .action(ArgAction::Append)
                .num_args(2)
                .value_names(["PATH", "VALUE"])
                .help("Set a value at the given path (e.g., 'key', 'items[0]', 'items[-]')"),
        )
        .arg(
            Arg::new("remove")
                .long("remove")
                .short('r')
                .action(ArgAction::Append)
                .value_name("PATH")
                .help("Remove a value at the given path"),
        )
        .arg(
            Arg::new("comment-set")
                .long("comment-set")
                .action(ArgAction::Append)
                .num_args(2)
                .value_names(["PATH", "TEXT"])
                .help("Set a comment at the given path"),
        )
        .arg(
            Arg::new("comment-append")
                .long("comment-append")
                .action(ArgAction::Append)
                .num_args(2)
                .value_names(["PATH", "TEXT"])
                .help("Append a comment at the given path"),
        )
        .arg(
            Arg::new("comment-remove")
                .long("comment-remove")
                .action(ArgAction::Append)
                .value_name("PATH")
                .help("Remove a comment at the given path"),
        )
        .arg(
            Arg::new("comment-style")
                .long("comment-style")
                .action(ArgAction::Append)
                .value_name("STYLE")
                .value_parser(["line", "hash", "block"])
                .help("Set comment style for subsequent comment operations"),
        )
        .arg(
            Arg::new("string-style")
                .long("string-style")
                .action(ArgAction::Append)
                .value_name("STYLE")
                .value_parser(["preserve", "double", "block", "unquoted"])
                .help("Set string style for subsequent set operations"),
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
                .value_parser(clap::value_parser!(std::path::PathBuf)),
        )
}

struct CliValueParser;

impl CliValueParser {
    fn parse_value(raw: &str) -> HjsonValue {
        match raw {
            "null" => return HjsonValue::Null,
            "true" => return HjsonValue::Bool(true),
            "false" => return HjsonValue::Bool(false),
            _ => {}
        }

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

    fn parse_comment_path(raw: &str) -> Result<HjsonCommentPath> {
        HjsonCommentPath::try_from(raw).with_context(|| format!("Invalid comment path: {raw}"))
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
        self.collect_set_ops()?;
        self.collect_remove_ops();
        self.collect_comment_ops()?;
        self.collect_style_ops()?;
        self.ops_with_indices.sort_by_key(|(idx, _)| *idx);
        Ok(self
            .ops_with_indices
            .into_iter()
            .map(|(_, op)| op)
            .collect())
    }

    fn collect_set_ops(&mut self) -> Result<()> {
        if let Some(sets) = self.matches.get_raw("set") {
            let vals: Vec<_> = sets.collect();
            let indices = self.collect_indices("set");

            for (chunk, idx_chunk) in vals.chunks(2).zip(indices.chunks(2)) {
                if chunk.len() != 2 {
                    continue;
                }
                let path = chunk[0].to_string_lossy().to_string();
                let value_str = chunk[1].to_string_lossy().to_string();
                let value = CliValueParser::parse_value(&value_str);
                self.ops_with_indices.push((
                    idx_chunk[0],
                    EditOp::Set {
                        path,
                        value,
                        string_style: CliValueParser::detect_inline_string_style(&value_str),
                    },
                ));
            }
        }

        Ok(())
    }

    fn collect_remove_ops(&mut self) {
        if let Some(removes) = self.matches.get_raw("remove") {
            let indices = self.collect_indices("remove");
            for (idx, val) in indices.iter().zip(removes) {
                self.ops_with_indices.push((
                    *idx,
                    EditOp::Remove {
                        path: val.to_string_lossy().to_string(),
                    },
                ));
            }
        }
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
                        path: CliValueParser::parse_comment_path(&chunk[0].to_string_lossy())
                            .expect("comment path should parse"),
                        text: chunk[1].to_string_lossy().to_string(),
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
                        path: CliValueParser::parse_comment_path(&chunk[0].to_string_lossy())
                            .expect("comment path should parse"),
                        text: chunk[1].to_string_lossy().to_string(),
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
                        path: CliValueParser::parse_comment_path(&val.to_string_lossy())
                            .expect("comment path should parse"),
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
}

pub fn run_edit(matches: &clap::ArgMatches) -> Result<()> {
    use std::path::PathBuf;

    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let mut doc = match HjsonDocument::parse(&content) {
        Ok(document) => document,
        Err(error) => anyhow::bail!("Parse errors:\n  {}", error),
    };

    let ops = collect_ops(matches)?;

    for op in ops {
        match op {
            EditOp::SetCommentStyle(style) => {
                doc = doc.comment_style(style);
            }
            EditOp::SetStringStyle(style) => {
                doc = doc.string_style(style);
            }
            EditOp::Set {
                path,
                value,
                string_style,
            } => {
                let style = string_style.unwrap_or(StringStyle::Preserve);
                doc = HjsonDocumentObjectEdit::set(doc.string_style(style), path, value)?;
            }
            EditOp::Remove { path } => {
                doc = HjsonDocumentObjectEdit::remove(doc, path)?;
            }
            EditOp::CommentSet { path, text } => {
                doc = HjsonDocumentCommentEdit::set(doc, path, text)?;
            }
            EditOp::CommentAppend { path, text } => {
                // getとsetを使ってappendを実装
                let existing = HjsonDocumentCommentEdit::get(&doc, path.clone())?;
                let new_comment = match existing {
                    Some(existing) => format!("{}\n{}", existing, text),
                    None => text,
                };
                doc = HjsonDocumentCommentEdit::set(doc, path, new_comment)?;
            }
            EditOp::CommentRemove { path } => {
                doc = HjsonDocumentCommentEdit::remove(doc, path)?;
            }
        }
    }

    let result = doc.to_string();
    let result = if content.ends_with("\r\n") && !result.ends_with("\r\n") {
        format!("{}\r\n", result)
    } else if content.ends_with('\n') && !result.ends_with('\n') {
        format!("{}\n", result)
    } else {
        result
    };
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

fn collect_ops(matches: &clap::ArgMatches) -> Result<Vec<EditOp>> {
    EditOpCollector::new(matches).collect()
}
