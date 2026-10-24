use rowan::GreenNode;

use super::{Format, FormatConfig};
use crate::ast::{self, AstNode, Value as AstValue};
use crate::syntax::SyntaxKind;

impl Format for AstValue {
    fn format_at(&self, config: &FormatConfig, depth: usize) -> GreenNode {
        match self.content() {
            Ok(content) => match content {
                ast::ValueContent::Object(obj) => {
                    let formatted = obj.format_at(config, depth + 1);
                    GreenNode::new(SyntaxKind::Value.into(), vec![formatted.into()])
                }
                ast::ValueContent::Array(arr) => {
                    let formatted = arr.format_at(config, depth + 1);
                    GreenNode::new(SyntaxKind::Value.into(), vec![formatted.into()])
                }
                ast::ValueContent::Scalar(_) => {
                    let indent = config.indent.render(depth);
                    let block = multiline_block_node(
                        self,
                        &format!("{indent}{}", config.indent.unit()),
                        indent.chars().count(),
                        &indent,
                        &config.newline,
                    );

                    block.unwrap_or_else(|| self.syntax().green().clone().into())
                }
            },
            Err(_) => self.syntax().green().clone().into(),
        }
    }
}

/// 複数行の Block 文字列を、字下げ付きの標準形で書く。
///
/// 本文の行に先頭の空白がある場合は、読み戻しでその空白が取り除かれないよう
/// `delimiter_column`（`'''` の桁）に合わせる。ない場合は `line_indent` を使うが、
/// `delimiter_column` を超えるときは `'''` の桁に合わせる。
///
/// 戻り値は `'''` を含む文字列全体。
pub(crate) fn block_string_text(
    body: &str,
    line_indent: &str,
    delimiter_column: usize,
    closing_indent: &str,
    newline: &str,
) -> String {
    let column_indent = " ".repeat(delimiter_column);
    let has_leading_whitespace = body.split('\n').any(|line| line.starts_with([' ', '\t']));
    let body_indent = if has_leading_whitespace || line_indent.chars().count() > delimiter_column {
        column_indent.as_str()
    } else {
        line_indent
    };

    let mut text = String::from("'''");
    for line in body.split('\n') {
        text.push_str(newline);
        if line.is_empty() {
            continue;
        }
        text.push_str(body_indent);
        text.push_str(line);
    }
    text.push_str(newline);
    text.push_str(closing_indent);
    text.push_str("'''");

    text
}

/// 値が複数行の Block 文字列なら、その本文を返す。
pub(crate) fn multiline_block_body(value: &AstValue) -> Option<String> {
    match value.content().ok()? {
        ast::ValueContent::Scalar(ast::ScalarValue::String(string)) => {
            if string.quote_style() != ast::QuoteStyle::Block || !string.raw_text().contains('\n') {
                return None;
            }

            Some(string.body_text().to_string())
        }
        _ => None,
    }
}

/// 複数行の Block 文字列を標準形にした green ノード。対象でなければ `None`。
pub(crate) fn multiline_block_node(
    value: &AstValue,
    line_indent: &str,
    delimiter_column: usize,
    closing_indent: &str,
    newline: &str,
) -> Option<GreenNode> {
    let body = multiline_block_body(value)?;
    let text = block_string_text(
        &body,
        line_indent,
        delimiter_column,
        closing_indent,
        newline,
    );

    Some(GreenNode::new(
        SyntaxKind::Value.into(),
        vec![SyntaxKind::BlockString.create_token(text.as_str()).into()],
    ))
}
