use clap::{Arg, ArgAction, ArgMatches, Command};

use anyhow::{Context, Result};
use hjson_edit::{
    ast::{CommentKind, QuoteStyle},
    high_level::{
        ops::{comment::HjsonDocumentCommentEdit, object::HjsonDocumentEdit},
        path::{HjsonCommentPath, HjsonObjectPath},
        style::detect_indent_from_ast,
        value::HjsonValue,
        verify::ValueEdit,
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
}

pub fn edit_command() -> Command {
    Command::new("edit")
        .about("Edit HJSON, keeping the existing layout unless --format is given")
        .arg(
            Arg::new("file")
                .required(true)
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .help("HJSON file to read (use - for standard input)"),
        )
        .arg(
            Arg::new("set")
                .long("set")
                .short('s')
                .action(ArgAction::Append)
                .num_args(2)
                .allow_hyphen_values(true)
                .value_names(["PATH", "VALUE"])
                .help("Set a value (VALUE is HJSON value text: 'hello', 8080, true, [1, 2], '{a: 1}')"),
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
                .allow_hyphen_values(true)
                .value_names(["PATH", "TEXT"])
                .help("Set a comment at the given path (e.g., 'key:before', 'items[0]:inline')"),
        )
        .arg(
            Arg::new("comment-append")
                .long("comment-append")
                .action(ArgAction::Append)
                .num_args(2)
                .allow_hyphen_values(true)
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
                .value_name("STYLE")
                .value_parser(["line", "hash", "block"])
                .help("Comment style used by comment operations"),
        )
        .arg(
            Arg::new("string-style")
                .long("string-style")
                .value_name("STYLE")
                .value_parser(["preserve", "double", "block", "unquoted"])
                .help(
                    "String style for set operations, overriding the quotes of the VALUE text \
                     (default: preserve)",
                ),
        )
        .arg(
            Arg::new("no-verify")
                .long("no-verify")
                .action(ArgAction::SetTrue)
                .help("Skip validating that edits keep values and comments (faster, less safe)"),
        )
        .arg(
            Arg::new("format")
                .long("format")
                .action(ArgAction::SetTrue)
                .help("Format the containers changed by the edit (default: keep the existing layout)"),
        )
        .arg(
            Arg::new("in-place")
                .long("in-place")
                .conflicts_with("output")
                .action(ArgAction::SetTrue)
                .help("Write the result back to the input file"),
        )
        .arg(
            Arg::new("output")
                .long("output")
                .conflicts_with("in-place")
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .help("Write the result to this file instead of stdout"),
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

/// `--newline` の指定値。
fn newline_override(matches: &ArgMatches) -> Option<&'static str> {
    matches
        .get_one::<String>("newline")
        .map(|newline| match newline.as_str() {
            "crlf" => "\r\n",
            _ => "\n",
        })
}

/// 編集で新しく作られた最も外側のコンテナ。
///
/// パスの先頭から見て、元の文書に無かった最初のセグメントまでを指す。
fn created_container(original: &HjsonDocument, path: &HjsonObjectPath) -> Option<HjsonObjectPath> {
    let segments = path.segments();
    for depth in 1..=segments.len() {
        let prefix = HjsonObjectPath::from_segments(segments[..depth].to_vec());
        if matches!(
            original.get(prefix.clone()),
            Err(HjsonEditError::PathNotFound(_))
        ) {
            return Some(prefix);
        }
    }

    None
}

/// コンテナの整形を行うか。`--format` か、整形オプションの明示指定で有効になる。
fn format_requested(matches: &ArgMatches) -> bool {
    matches.get_flag("format")
        || matches.value_source("indent-style") == Some(clap::parser::ValueSource::CommandLine)
        || matches.value_source("indent-width") == Some(clap::parser::ValueSource::CommandLine)
        || matches.value_source("newline") == Some(clap::parser::ValueSource::CommandLine)
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

/// コメントパスを解釈する。
fn parse_comment_path(raw: &str) -> Result<HjsonCommentPath> {
    Ok(HjsonCommentPath::try_from(raw)?)
}

/// オブジェクトパスを解釈する。
fn parse_object_path(raw: &str) -> Result<HjsonObjectPath> {
    Ok(HjsonObjectPath::parse(raw.to_string())?)
}

/// `--set` の値を HJSON の値として解釈する。
///
/// 文書と同じ文法を使うため、引用符の有無がそのまま値の種類になる。
fn parse_value(raw: &str) -> Result<HjsonValue> {
    if raw.trim().is_empty() {
        anyhow::bail!("Invalid value: the value text is empty (use \"\" for an empty string)");
    }

    let document = HjsonDocument::parse(raw).with_context(|| format!("Invalid value: {raw}"))?;
    if contains_comment(document.syntax_root()) {
        anyhow::bail!(
            "Invalid value: comments are not allowed in the value text (they are dropped)"
        );
    }

    let path = HjsonObjectPath::parse(String::new()).expect("the empty path is always valid");

    document
        .get(path)
        .with_context(|| format!("Invalid value: {raw}"))
}

/// 構文木にコメントを含むか。
fn contains_comment(root: &hjson_edit::RedNode) -> bool {
    use rowan::NodeOrToken;

    root.descendants_with_tokens().any(|element| match element {
        NodeOrToken::Token(token) => token.kind().is_comment(),
        NodeOrToken::Node(node) => node.kind().is_comment(),
    })
}

/// 値テキストの引用スタイル（`"..."` / `'...'` / `'''...'''`）。指定があればその値だけに使う。
///
/// 希望として扱い、そのスタイルで書けない値は二重引用符へ降格させる。
fn inline_string_style(raw: &str) -> Option<Prefer<QuoteStyle>> {
    if raw.len() >= 6 && raw.starts_with("'''") && raw.ends_with("'''") {
        return Some(Prefer::Preferred(QuoteStyle::Block));
    }
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        return Some(Prefer::Preferred(QuoteStyle::DoubleQuoted));
    }
    if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        return Some(Prefer::Preferred(QuoteStyle::SingleQuoted));
    }
    None
}

/// 値の引用スタイル。
fn parse_string_style(raw: &str) -> Result<Prefer<QuoteStyle>> {
    Ok(match raw {
        "preserve" => Prefer::Preserve,
        "double" => Prefer::Explicit(QuoteStyle::DoubleQuoted),
        "block" => Prefer::Explicit(QuoteStyle::Block),
        "unquoted" => Prefer::Explicit(QuoteStyle::Unquoted),
        other => anyhow::bail!("Invalid string style: {other}"),
    })
}

/// コメントのスタイル。
fn parse_comment_style(raw: &str) -> Result<Prefer<CommentKind>> {
    Ok(match raw {
        "line" => Prefer::Explicit(CommentKind::Line),
        "hash" => Prefer::Explicit(CommentKind::Hash),
        "block" => Prefer::Explicit(CommentKind::Block),
        other => anyhow::bail!("Invalid comment style: {other}"),
    })
}

/// 引数の並び順で編集操作を集める。
///
/// スタイル指定はその回全体に適用するため、ここでは扱わない。
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
        self.collect_comment_set_ops()?;
        self.collect_comment_append_ops()?;
        self.collect_comment_remove_ops()?;
        self.ops_with_indices.sort_by_key(|(index, _)| *index);

        Ok(self
            .ops_with_indices
            .into_iter()
            .map(|(_, op)| op)
            .collect())
    }

    fn collect_set_ops(&mut self) -> Result<()> {
        for (index, values) in self.pairs("set") {
            if values.len() != 2 {
                continue;
            }
            self.ops_with_indices.push((
                index,
                EditOp::Set {
                    path: values[0].clone(),
                    value: parse_value(&values[1])?,
                    string_style: inline_string_style(&values[1]),
                },
            ));
        }

        Ok(())
    }

    fn collect_remove_ops(&mut self) {
        for (index, values) in self.pairs("remove") {
            if let Some(path) = values.first() {
                self.ops_with_indices
                    .push((index, EditOp::Remove { path: path.clone() }));
            }
        }
    }

    fn collect_comment_set_ops(&mut self) -> Result<()> {
        for (index, values) in self.pairs("comment-set") {
            if values.len() != 2 {
                continue;
            }
            self.ops_with_indices.push((
                index,
                EditOp::CommentSet {
                    path: parse_comment_path(&values[0])?,
                    text: values[1].clone(),
                },
            ));
        }

        Ok(())
    }

    fn collect_comment_append_ops(&mut self) -> Result<()> {
        for (index, values) in self.pairs("comment-append") {
            if values.len() != 2 {
                continue;
            }
            self.ops_with_indices.push((
                index,
                EditOp::CommentAppend {
                    path: parse_comment_path(&values[0])?,
                    text: values[1].clone(),
                },
            ));
        }

        Ok(())
    }

    fn collect_comment_remove_ops(&mut self) -> Result<()> {
        for (index, values) in self.pairs("comment-remove") {
            if let Some(path) = values.first() {
                self.ops_with_indices.push((
                    index,
                    EditOp::CommentRemove {
                        path: parse_comment_path(path)?,
                    },
                ));
            }
        }

        Ok(())
    }

    /// 引数名ごとに「出現位置」と「値の並び（出現ごと）」を返す。
    fn pairs(&self, name: &str) -> Vec<(usize, Vec<String>)> {
        let Some(raw) = self.matches.get_raw(name) else {
            return Vec::new();
        };
        let values: Vec<String> = raw
            .map(|value| value.to_string_lossy().to_string())
            .collect();
        let indices: Vec<usize> = self
            .matches
            .indices_of(name)
            .map(|indices| indices.collect())
            .unwrap_or_default();

        group_by_index(
            indices,
            values,
            self.matches.get_one::<String>(name).is_some(),
        )
    }
}

/// `num_args(2)` の引数を、出現ごとの (先頭 index, 値の並び) にまとめる。
fn group_by_index(
    indices: Vec<usize>,
    values: Vec<String>,
    _is_single: bool,
) -> Vec<(usize, Vec<String>)> {
    let mut grouped: Vec<(usize, Vec<String>)> = Vec::new();
    for (index, value) in indices.into_iter().zip(values) {
        match grouped.last_mut() {
            // 同じ出現の 2 つ目以降の値は、直前の出現に足す
            Some((previous_start, collected)) if index == *previous_start + collected.len() => {
                collected.push(value);
            }
            _ => grouped.push((index, vec![value])),
        }
    }

    grouped
}

pub fn run_edit(matches: &clap::ArgMatches) -> Result<()> {
    use std::path::PathBuf;

    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    super::reject_in_place_stdin(matches, path)?;
    let content = super::read_input(path)?;

    let mut doc = match HjsonDocument::parse(&content) {
        Ok(document) => {
            super::warn_skipped_input(&document);
            document
        }
        Err(error) => anyhow::bail!("{error}"),
    };

    let ops = EditOpCollector::new(matches).collect()?;
    let config = format_config(matches, &doc);
    // `--string-style` の指定は値テキストの引用より優先する。無ければ値テキストを使う。
    let string_style = matches
        .get_one::<String>("string-style")
        .map(|style| parse_string_style(style))
        .transpose()?;

    // 値の操作は、操作ごとに編集した値だけを検証し、最後に文書全体を 1 回だけ検証する。
    // コメント操作は従来どおり操作ごとに検証する。
    let verify = !matches.get_flag("no-verify");
    doc = doc
        .verify_edits(false)
        .format_config(config)
        .string_style(string_style.unwrap_or_default());
    if let Some(style) = matches.get_one::<String>("comment-style") {
        doc = doc.comment_style(parse_comment_style(style)?);
    }

    if ops.is_empty() {
        return write_output(matches, path, &doc.to_string(), &content);
    }

    let original = doc.clone();
    let mut edited_slots: Vec<HjsonObjectPath> = Vec::new();
    let mut value_edits: Vec<ValueEdit> = Vec::new();
    for op in ops {
        match op {
            EditOp::Set {
                path,
                value,
                string_style: inline_style,
            } => {
                let parsed = parse_object_path(&path)?;
                // 値テキストの引用スタイルは、その値だけに使う（`--string-style` があれば使わない）
                let styled = match (string_style, inline_style) {
                    (None, Some(style)) => doc.string_style(style),
                    _ => doc,
                };
                doc = styled.set(parsed.clone(), value.clone())?;
                doc = doc.string_style(string_style.unwrap_or_default());
                value_edits.push(ValueEdit::Set(parsed.clone(), value));
                edited_slots.push(parsed);
            }
            EditOp::Remove { path } => {
                let parsed = parse_object_path(&path)?;
                doc = doc.remove(parsed.clone())?;
                value_edits.push(ValueEdit::Remove(parsed));
            }
            EditOp::CommentSet { path, text } => {
                doc = doc.verify_edits(verify).set_comment(path, text)?;
                doc = doc.verify_edits(false);
            }
            EditOp::CommentAppend { path, text } => {
                doc = doc.verify_edits(verify).append_comment(path, text)?;
                doc = doc.verify_edits(false);
            }
            EditOp::CommentRemove { path } => {
                doc = doc.verify_edits(verify).remove_comment(path)?;
                doc = doc.verify_edits(false);
            }
        }
    }

    if verify && !value_edits.is_empty() {
        doc.verify_value_edits(&original, &value_edits)?;
    }

    // ここから先は整形だけを行う。整形も値とコメントを変えてはいけないので、
    // まとめて整形した後に 1 回だけ検証する（コンテナごとの再パースを避ける）。
    let before_formatting = doc.clone();

    // 既定は保存重視: 複数行になった要素だけを整える。--format（または整形オプションの
    // 明示指定）があるときは、変更したコンテナを整形する。
    let config = doc.current_format_config();
    if format_requested(matches) {
        for container in doc.touched_containers().to_vec() {
            match doc.format_container(&container, &config) {
                Ok(updated) => doc = updated,
                Err(
                    HjsonEditError::PathNotFound(_) | HjsonEditError::UnexpectedStructure { .. },
                ) => {}
                Err(error) => return Err(error.into()),
            }
        }
    } else {
        // 編集で新しく作られたコンテナは既存の体裁が無いので整形する。
        // 行の途中に作られた場合は、インラインの配置を保つため触らない。
        for slot in &edited_slots {
            if let Some(created) = created_container(&original, slot) {
                if !doc.starts_at_line(&created)? {
                    continue;
                }
                match doc.format_container(&created, &config) {
                    Ok(updated) => doc = updated,
                    Err(
                        HjsonEditError::PathNotFound(_)
                        | HjsonEditError::UnexpectedStructure { .. },
                    ) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }

        for slot in edited_slots {
            match doc.format_slot(&slot, &config) {
                Ok(updated) => doc = updated,
                Err(
                    HjsonEditError::PathNotFound(_) | HjsonEditError::UnexpectedStructure { .. },
                ) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }

    if verify {
        doc.verify_formatting(&before_formatting)?;
    }

    write_output(matches, path, &doc.to_string(), &content)
}

/// 編集結果を、末尾の改行を保ったまま出力する。
fn write_output(
    matches: &ArgMatches,
    path: &std::path::Path,
    result: &str,
    content: &str,
) -> Result<()> {
    use std::path::PathBuf;

    let result = match newline_override(matches) {
        // 文書末尾の改行はコンテナの外側にあるため、指定があれば揃える
        Some(newline) if content.ends_with('\n') => {
            let body = result
                .strip_suffix("\r\n")
                .or_else(|| result.strip_suffix('\n'))
                .unwrap_or(result);
            format!("{body}{newline}")
        }
        _ if content.ends_with('\n') && !result.ends_with('\n') => {
            format!("{result}\n")
        }
        _ => result.to_string(),
    };

    if matches.get_flag("in-place") {
        super::write_atomic(path, &result)?;
    } else if let Some(output_path) = matches.get_one::<PathBuf>("output") {
        super::write_atomic(output_path, &result)?;
    } else {
        print!("{result}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_value_kinds() {
        assert!(matches!(
            parse_value("true").unwrap(),
            HjsonValue::Bool(true)
        ));
        assert!(matches!(
            parse_value("false").unwrap(),
            HjsonValue::Bool(false)
        ));
        assert!(matches!(parse_value("null").unwrap(), HjsonValue::Null));
        assert!(matches!(
            parse_value("123").unwrap(),
            HjsonValue::Integer(123)
        ));
        assert!(matches!(parse_value("2.5").unwrap(), HjsonValue::Float(n) if n == 2.5));
    }

    #[test]
    fn test_parse_value_strings_and_containers() {
        assert!(matches!(parse_value("hello").unwrap(), HjsonValue::String(s) if s == "hello"));
        assert!(matches!(parse_value("\"true\"").unwrap(), HjsonValue::String(s) if s == "true"));
        assert!(matches!(parse_value("'''a'''").unwrap(), HjsonValue::String(s) if s == "a"));
        assert!(
            matches!(parse_value("[1, 2]").unwrap(), HjsonValue::Array(items) if items.len() == 2)
        );
        assert!(
            matches!(parse_value("{a: 1}").unwrap(), HjsonValue::Object(entries) if entries.len() == 1)
        );
    }

    #[test]
    fn test_parse_value_rejects_empty_text() {
        assert!(parse_value("").is_err());
        assert!(parse_value("   ").is_err());
    }

    #[test]
    fn test_inline_string_style_detection() {
        assert_eq!(inline_string_style("hello"), None);
        assert_eq!(
            inline_string_style("\"x\""),
            Some(Prefer::Preferred(QuoteStyle::DoubleQuoted))
        );
        assert_eq!(
            inline_string_style("'x'"),
            Some(Prefer::Preferred(QuoteStyle::SingleQuoted))
        );
        assert_eq!(
            inline_string_style("'''x'''"),
            Some(Prefer::Preferred(QuoteStyle::Block))
        );
    }
}
