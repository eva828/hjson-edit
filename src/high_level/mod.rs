//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

pub(crate) mod error;
pub(crate) mod ops;
pub(crate) mod path;
pub(crate) mod style;
pub(crate) mod value;

use crate::document_style::{DetectNewline, DocumentStyle, NewlineKind};
use crate::high_level::error::HjsonEditError;
use crate::syntax::lexer::Lexer;
use crate::syntax::parser::Parser;
use crate::ParseError;
use line_index::LineIndex;
use rowan::GreenNode;

/// HJSONドキュメント
#[derive(Debug, Clone)]
pub struct HjsonDocument {
    green: GreenNode,
}

impl HjsonDocument {
    /// HJSON文字列をパースしてドキュメントを作成
    pub fn parse(input: &str) -> Result<Self, HjsonEditError> {
        let lex_result = Lexer::new(input).lex();

        // Lexerエラーがある場合、それをParseErrorに変換して即座に返す
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

        Ok(Self {
            green: result.green,
        })
    }

    pub fn green_node(&self) -> &GreenNode {
        &self.green
    }

    pub fn into_green_node(self) -> GreenNode {
        self.green
    }
}
