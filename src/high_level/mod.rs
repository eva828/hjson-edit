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

impl HjsonDocument {
    /// HJSON文字列をパースしてドキュメントを作成
    pub fn parse(input: &str) -> Result<Self, HjsonEditError> {
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

    pub fn to_string(&self) -> String {
        self.root.to_string()
    }
}
