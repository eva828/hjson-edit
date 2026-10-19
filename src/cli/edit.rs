use clap::{Arg, ArgAction, ArgMatches, Command};

use anyhow::{Context, Result};
use hjson_edit::{
    ast::{CommentKind, QuoteStyle},
    high_level::{
        ops::{comment::HjsonDocumentCommentEdit, object::HjsonDocumentObjectEdit},
        path::{HjsonCommentPath, HjsonObjectPath},
        style::detect_indent_from_ast,
        value::HjsonValue,
        HjsonDocument, Prefer,
    },
    FormatConfig, HjsonEditError, IndentStyle,
};

#[derive(Debug, Clone)]
enum EditOp {
    Set {
        path: String,
        value: HjsonValue,
        string_style: Option<Prefer<QuoteStyle>>,
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
    SetCommentStyle(Prefer<CommentKind>),
    SetStringStyle(Prefer<QuoteStyle>),
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
        .arg(
            Arg::new("indent-style")
                .long("indent-style")
                .value_parser(["spaces", "tabs"])
                .help("Indent style for the formatted containers (default: detected from the document)"),
        )
        .arg(
            Arg::new("indent-width")
                .long("indent-width")
                .value_parser(clap::value_parser!(u8).range(1..=8))
                .help("Indent width in spaces for the formatted containers (default: detected)"),
        )
        .arg(
            Arg::new("newline")
                .long("newline")
                .value_parser(["lf", "crlf"])
                .help("Newline style for the formatted containers (default: detected from the document)"),
        )
}

/// 編集で変更したコンテナを記録する。
///
/// 編集の核心は最小限のテキスト変更に留め、体裁は変更したコンテナの部分整形で整える。
fn push_container(containers: &mut Vec<HjsonObjectPath>, path: &HjsonObjectPath) {
    if let Some((parent, _)) = path.split_last() {
        containers.push(parent);
    }
}

/// `set` が実際に書き込むコンテナ（パスのうち実在する最深の祖先）を返す。
///
/// 中間のコンテナが無い場合は、実在する最長の prefix の位置へまとめて挿入されるため、
/// その祖先が「変更されたコンテナ」になる。
fn insertion_container(doc: &HjsonDocument, path: &HjsonObjectPath) -> HjsonObjectPath {
    let Some((parent, _)) = path.split_last() else {
        return path.clone();
    };

    for depth in (1..=parent.depth()).rev() {
        let Some((candidate, _)) = parent.split_at(depth) else {
            break;
        };
        if doc.get(candidate.clone()).is_ok() {
            return candidate;
        }
    }

    // どの prefix も実在しない場合はルート（文書全体）へ挿入される
    HjsonObjectPath::from_segments(Vec::new())
}

/// コメント操作が触れたコンテナを記録する。
///
/// ヘッダー/フッターは文書全体に付くため、コンテナの整形は不要。
fn push_comment_container(containers: &mut Vec<HjsonObjectPath>, path: &HjsonCommentPath) {
    match path {
        HjsonCommentPath::Header | HjsonCommentPath::Footer => {}
        HjsonCommentPath::Before(path) | HjsonCommentPath::Inline(path) => {
            push_container(containers, path);
        }
    }
}

/// 互いに祖先・子孫の関係にあるコンテナは外側だけを残す。
fn simplify_containers(mut containers: Vec<HjsonObjectPath>) -> Vec<HjsonObjectPath> {
    containers.sort_by_key(|path| path.depth());
    containers.dedup();

    let mut kept: Vec<HjsonObjectPath> = Vec::new();
    for path in containers {
        let covered = kept.iter().any(|outer| {
            let outer_segments = outer.segments();
            outer_segments.len() <= path.segments().len()
                && path.segments().starts_with(outer_segments)
        });
        if !covered {
            kept.push(path);
        }
    }
    kept
}

/// `--newline` の指定値。
fn newline_override(matches: &ArgMatches) -> Option<&'static str> {
    matches
        .get_one::<String>("newline")
        .map(|newline| match newline.as_str() {
            "crlf" => "\r\n",
            _ => "\n",
        })
}

/// 文書から検出したインデント幅（タブの場合は既定の 2）。
fn detected_indent_width(doc: &HjsonDocument) -> u8 {
    match detect_indent_from_ast(doc.syntax_root()) {
        IndentStyle::Spaces(width) => width,
        IndentStyle::Tabs => 2,
    }
}

/// CLI 引数と文書から整形設定を決める。未指定の項目は文書から推定する。
fn format_config(matches: &ArgMatches, doc: &HjsonDocument) -> FormatConfig {
    let mut config = doc.detected_format_config();
    let width = matches.get_one::<u8>("indent-width").copied();
    let indent = match matches
        .get_one::<String>("indent-style")
        .map(String::as_str)
    {
        Some("tabs") => Some(IndentStyle::Tabs),
        // 幅の指定が無ければ文書から検出した幅を使う
        Some("spaces") => Some(IndentStyle::Spaces(
            width.unwrap_or_else(|| detected_indent_width(doc)),
        )),
        // clap が値を制限しているため到達しない
        Some(_) => None,
        None => width.map(IndentStyle::Spaces),
    };
    if let Some(indent) = indent {
        config = config.with_indent(indent);
    }
    if let Some(newline) = newline_override(matches) {
        config = config.with_newline(newline);
    }
    config
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

    fn detect_inline_string_style(raw: &str) -> Option<Prefer<QuoteStyle>> {
        if raw.len() >= 6 && raw.starts_with("'''") && raw.ends_with("'''") {
            return Some(Prefer::Explicit(QuoteStyle::Block));
        }
        if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
            return Some(Prefer::Explicit(QuoteStyle::DoubleQuoted));
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
        self.collect_comment_set_ops()?;
        self.collect_comment_append_ops()?;
        self.collect_comment_remove_ops()?;
        Ok(())
    }

    fn collect_comment_set_ops(&mut self) -> Result<()> {
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
                        path: CliValueParser::parse_comment_path(&chunk[0].to_string_lossy())?,
                        text: chunk[1].to_string_lossy().to_string(),
                    },
                ));
            }
        }
        Ok(())
    }

    fn collect_comment_append_ops(&mut self) -> Result<()> {
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
                        path: CliValueParser::parse_comment_path(&chunk[0].to_string_lossy())?,
                        text: chunk[1].to_string_lossy().to_string(),
                    },
                ));
            }
        }
        Ok(())
    }

    fn collect_comment_remove_ops(&mut self) -> Result<()> {
        if let Some(removes) = self.matches.get_raw("comment-remove") {
            let indices = self.collect_indices("comment-remove");
            for (idx, val) in indices.iter().zip(removes) {
                self.ops_with_indices.push((
                    *idx,
                    EditOp::CommentRemove {
                        path: CliValueParser::parse_comment_path(&val.to_string_lossy())?,
                    },
                ));
            }
        }
        Ok(())
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
                    "line" => Prefer::Explicit(CommentKind::Line),
                    "hash" => Prefer::Explicit(CommentKind::Hash),
                    "block" => Prefer::Explicit(CommentKind::Block),
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
                    "preserve" => Prefer::Preserve,
                    "double" => Prefer::Explicit(QuoteStyle::DoubleQuoted),
                    "block" => Prefer::Explicit(QuoteStyle::Block),
                    "unquoted" => Prefer::Explicit(QuoteStyle::Unquoted),
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
    let content = super::read_input(path)?;

    let mut doc = match HjsonDocument::parse(&content) {
        Ok(document) => {
            super::warn_skipped_input(&document);
            document
        }
        Err(error) => anyhow::bail!("{error}"),
    };

    let ops = collect_ops(matches)?;

    if ops.is_empty() {
        if matches.get_flag("in-place") {
            std::fs::write(path, &content)
                .with_context(|| format!("Failed to write file: {}", path.display()))?;
        } else if let Some(output_path) = matches.get_one::<PathBuf>("output") {
            std::fs::write(output_path, &content)
                .with_context(|| format!("Failed to write file: {}", output_path.display()))?;
        } else {
            print!("{}", content);
        }

        return Ok(());
    }

    let mut changed_containers: Vec<HjsonObjectPath> = Vec::new();

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
                let parsed = HjsonObjectPath::parse(&path)
                    .with_context(|| format!("Invalid path: {path}"))?;
                changed_containers.push(insertion_container(&doc, &parsed));
                // 値テキストのインラインスタイル（"..." / '''...'''）だけを上書きに使い、
                // 指定が無ければ --string-style で設定した文書のスタイルを保つ。
                let doc_with_style = match string_style {
                    Some(style) => doc.string_style(style),
                    None => doc,
                };
                doc = doc_with_style.set(parsed, value)?;
            }
            EditOp::Remove { path } => {
                let parsed = HjsonObjectPath::parse(&path)
                    .with_context(|| format!("Invalid path: {path}"))?;
                push_container(&mut changed_containers, &parsed);
                doc = doc.remove(parsed)?;
            }
            EditOp::CommentSet { path, text } => {
                push_comment_container(&mut changed_containers, &path);
                doc = doc.set_comment(path, text)?;
            }
            EditOp::CommentAppend { path, text } => {
                push_comment_container(&mut changed_containers, &path);
                doc = doc.append_comment(path, text)?;
            }
            EditOp::CommentRemove { path } => {
                push_comment_container(&mut changed_containers, &path);
                doc = doc.remove_comment(path)?;
            }
        }
    }

    // 変更したコンテナだけを整形する。編集で消えた・コンテナでなくなった対象は対象外。
    let config = format_config(matches, &doc);
    for container in simplify_containers(changed_containers) {
        match doc.format_container(&container, &config) {
            Ok(updated) => doc = updated,
            Err(HjsonEditError::PathNotFound(_) | HjsonEditError::UnexpectedStructure { .. }) => {}
            Err(error) => return Err(error.into()),
        }
    }

    let result = doc.to_string();
    let result = match newline_override(matches) {
        // 文書末尾の改行はコンテナの外側にあるため、指定があれば揃える
        Some(newline) if content.ends_with('\n') => {
            let body = result
                .strip_suffix("\r\n")
                .or_else(|| result.strip_suffix('\n'))
                .unwrap_or(&result);
            format!("{body}{newline}")
        }
        _ if content.ends_with('\n') && !result.ends_with('\n') => {
            format!("{}{}", result, doc.current_newline())
        }
        _ => result,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_value_parser_keywords_and_numbers() {
        assert!(matches!(
            CliValueParser::parse_value("true"),
            HjsonValue::Bool(true)
        ));
        assert!(matches!(
            CliValueParser::parse_value("false"),
            HjsonValue::Bool(false)
        ));
        assert!(matches!(
            CliValueParser::parse_value("null"),
            HjsonValue::Null
        ));

        assert!(matches!(
            CliValueParser::parse_value("123"),
            HjsonValue::Integer(123)
        ));
        let expected: f64 = "3.14".parse().unwrap();
        assert!(
            matches!(CliValueParser::parse_value("3.14"), HjsonValue::Float(n) if n == expected)
        );
    }

    #[test]
    fn test_cli_value_parser_string_edge_cases() {
        assert!(matches!(CliValueParser::parse_value(""), HjsonValue::String(s) if s.is_empty()));
        assert!(matches!(CliValueParser::parse_value("\""), HjsonValue::String(s) if s == "\""));
        assert!(
            matches!(CliValueParser::parse_value("\"\""), HjsonValue::String(s) if s.is_empty())
        );
        assert!(
            matches!(CliValueParser::parse_value("\"foo"), HjsonValue::String(s) if s == "\"foo")
        );
        assert!(matches!(CliValueParser::parse_value("'''"), HjsonValue::String(s) if s == "'''"));
        assert!(
            matches!(CliValueParser::parse_value("''''''"), HjsonValue::String(s) if s.is_empty())
        );
    }

    #[test]
    fn test_cli_value_parser_multibyte() {
        assert!(
            matches!(CliValueParser::parse_value("\"あ\""), HjsonValue::String(s) if s == "あ")
        );
        assert!(
            matches!(CliValueParser::parse_value("'''あ'''"), HjsonValue::String(s) if s == "あ")
        );
    }
}
