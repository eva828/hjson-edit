//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

pub mod error;
pub mod ext;
pub mod ops;
pub mod path;
pub mod style;
pub mod value;
pub mod walker;

use crate::ast as semantic_ast;
use crate::ast::AstNode;
use crate::high_level::error::HjsonEditError;
use crate::high_level::ops::comment::CommentStyle;
use crate::high_level::ops::object::StringStyle;
use crate::high_level::style::DetectNewline;
use crate::syntax::lexer::Lexer;
use crate::syntax::parser::Parser;
use crate::RedNode;
use line_index::LineIndex;

/// HJSONドキュメント
#[derive(Debug, Clone)]
pub struct HjsonDocument {
    root: RedNode,
    string_style: StringStyle,
    comment_style: CommentStyle,
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

        let parser = Parser::new(lex_result.tokens);
        let result = parser.parse();

        if !result.errors.is_empty() {
            return Err(HjsonEditError::ParseError {
                errors: result.errors,
                line_index: LineIndex::new(input),
            });
        }

        let root = RedNode::new_root(result.green);
        // let root = RedNode::new_root(result.green).clone_for_update();

        Ok(Self {
            root,
            string_style: StringStyle::default(),
            comment_style: CommentStyle::default(),
            newline: input.detect_newline().as_str().to_string(),
        })
    }

    pub fn string_style(mut self, style: StringStyle) -> Self {
        self.string_style = style;
        self
    }

    pub fn comment_style(mut self, style: CommentStyle) -> Self {
        self.comment_style = style;
        self
    }

    pub fn current_string_style(&self) -> StringStyle {
        self.string_style
    }

    pub fn current_comment_style(&self) -> CommentStyle {
        self.comment_style
    }

    pub fn current_newline(&self) -> &str {
        &self.newline
    }

    pub fn root(&self) -> &RedNode {
        &self.root
    }

    pub fn ast(&self) -> Result<semantic_ast::Document, HjsonEditError> {
        semantic_ast::Document::cast(self.root.clone())
    }

    pub fn to_string(&self) -> String {
        self.root.to_string()
    }
}
