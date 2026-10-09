//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

pub mod error;
pub mod ops;
pub mod path;
pub(crate) mod resolve;
pub mod style;
pub mod value;

use crate::ast as semantic_ast;
use crate::ast::{AstNode, CommentKind, QuoteStyle};
use crate::high_level::error::HjsonEditError;
use crate::high_level::ops::object::AppendMode;
use crate::high_level::style::detect_newline_from_ast;
use crate::syntax::lexer::Lexer;
use crate::syntax::parser::Parser;
use crate::syntax::MAX_INPUT_BYTES;
use crate::RedNode;
use line_index::LineIndex;
use rowan::GreenNode;

/// 値の維持（Preserve）または明示的な指定（Explicit）を表す型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Prefer<T> {
    #[default]
    Preserve,
    Explicit(T),
}

/// HJSONドキュメント
#[derive(Debug, Clone)]
pub struct HjsonDocument {
    root: RedNode,
    string_style: Prefer<QuoteStyle>,
    comment_style: Prefer<CommentKind>,
    append_mode: AppendMode,
    newline: String,
}

/// 入力長が上限（`MAX_INPUT_BYTES`）以内かを検査する。
///
/// 位置情報が u32 のため、それを超える入力は正しい位置を報告できない。
pub(crate) fn check_input_len(len: usize) -> Result<(), HjsonEditError> {
    if len > MAX_INPUT_BYTES {
        return Err(HjsonEditError::InputTooLong {
            len,
            limit: MAX_INPUT_BYTES,
        });
    }

    Ok(())
}

impl HjsonDocument {
    /// HJSON文字列をパースしてドキュメントを作成
    pub fn parse(input: &str) -> Result<Self, HjsonEditError> {
        check_input_len(input.len())?;

        let lex_result = Lexer::new(input).lex();

        if !lex_result.errors.is_empty() {
            return Err(HjsonEditError::LexError {
                errors: lex_result.errors,
                line_index: LineIndex::new(input),
            });
        }

        let parser = Parser::new(input, lex_result.tokens);
        let result = parser.parse();

        if !result.errors.is_empty() {
            return Err(HjsonEditError::ParseError {
                errors: result.errors,
                line_index: LineIndex::new(input),
            });
        }

        let root = RedNode::new_root(result.green);
        let newline = detect_newline_from_ast(&root).as_str().to_string();

        Ok(Self {
            root,
            string_style: Prefer::default(),
            comment_style: Prefer::default(),
            append_mode: AppendMode::default(),
            newline,
        })
    }

    pub fn string_style(mut self, style: Prefer<QuoteStyle>) -> Self {
        self.string_style = style;
        self
    }

    pub fn comment_style(mut self, style: Prefer<CommentKind>) -> Self {
        self.comment_style = style;
        self
    }

    pub fn append_mode(mut self, mode: AppendMode) -> Self {
        self.append_mode = mode;
        self
    }

    pub(crate) fn root(self, root: GreenNode) -> HjsonDocument {
        HjsonDocument {
            root: RedNode::new_root(root),
            ..self
        }
    }

    pub(crate) fn current_string_style(&self) -> Prefer<QuoteStyle> {
        self.string_style
    }

    pub(crate) fn current_comment_style(&self) -> Prefer<CommentKind> {
        self.comment_style
    }

    pub(crate) fn current_append_mode(&self) -> AppendMode {
        self.append_mode
    }

    pub fn current_newline(&self) -> &str {
        &self.newline
    }

    pub fn syntax_root(&self) -> &RedNode {
        &self.root
    }

    pub fn ast(&self) -> Result<semantic_ast::Document, HjsonEditError> {
        semantic_ast::Document::cast(self.root.clone())
    }
}

impl std::fmt::Display for HjsonDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_length_at_limit_is_accepted() {
        assert!(check_input_len(MAX_INPUT_BYTES).is_ok());
    }

    #[test]
    fn input_length_over_limit_is_rejected() {
        let result = check_input_len(MAX_INPUT_BYTES + 1);
        assert!(matches!(
            result,
            Err(HjsonEditError::InputTooLong { len, limit })
                if len == MAX_INPUT_BYTES + 1 && limit == MAX_INPUT_BYTES
        ));
    }
}
