//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

pub mod ast;
pub mod error;
pub mod ext;
pub mod ops;
pub mod path;
pub mod style;
pub mod value;
pub mod walker;

use crate::high_level::error::HjsonEditError;
use crate::high_level::ops::comment::CommentStyle;
use crate::high_level::ops::object::StringStyle;
use crate::syntax::lexer::Lexer;
use crate::syntax::parser::Parser;
use crate::RedNode;
use line_index::LineIndex;

/// HJSONドキュメント
#[derive(Debug, Clone)]
pub struct HjsonDocument {
    root: RedNode,
    string_style: StringStyle,
    commnt_style: CommentStyle,
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

        let root = RedNode::new_root(result.green).clone_for_update();

        Ok(Self {
            root,
            string_style: StringStyle::default(),
            commnt_style: CommentStyle::default(),
        })
    }

    pub fn string_style(mut self, style: StringStyle) -> Self {
        self.string_style = style;
        self
    }

    pub fn comment_style(mut self, style: CommentStyle) -> Self {
        self.commnt_style = style;
        self
    }

    pub fn root(&self) -> &RedNode {
        &self.root
    }

    pub fn to_string(&self) -> String {
        self.root.to_string()
    }
}
