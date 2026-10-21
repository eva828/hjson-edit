//! 高レベルAPIモジュール
//!
//! 低レベルAPI（Green/Red Tree操作）をラップし、より使いやすいドットパスベースのAPIを提供する。

/// 編集APIのエラー型。
pub mod error;
mod format;
pub(crate) mod indent;
/// 文書を編集する操作（値・コメント・マージ）。
pub mod ops;
/// 値とコメントの位置を指すパス。
pub mod path;
pub(crate) mod resolve;
/// 文書の体裁とスタイルの推定。
pub mod style;
/// 値の型と描画。
pub mod value;
mod value_ops;
mod verify;

use crate::ast as semantic_ast;
use crate::ast::{AstNode, CommentKind, QuoteStyle};
use crate::format::FormatConfig;
use crate::high_level::error::HjsonEditError;
use crate::high_level::path::HjsonObjectPath;
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
    /// 元のスタイルを保つ。
    #[default]
    Preserve,
    /// 指定したスタイルを使う。
    Explicit(T),
}

/// HJSONドキュメント
#[derive(Debug, Clone)]
pub struct HjsonDocument {
    root: RedNode,
    string_style: Prefer<QuoteStyle>,
    comment_style: Prefer<CommentKind>,
    verify: bool,
    format: Option<FormatConfig>,
    touched: Vec<HjsonObjectPath>,
    newline: String,
    warnings: Vec<String>,
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

        // 閉じないブロックコメントは基準実装（hjson-cs）と同じく読み飛ばし、
        // 警告として保持する（切断の検出は CLI の役割）。
        let (warnings, fatal): (Vec<_>, Vec<_>) = lex_result
            .errors
            .iter()
            .partition(|error| error.message == "Unterminated block comment");

        if !fatal.is_empty() {
            return Err(HjsonEditError::LexError {
                errors: fatal.into_iter().cloned().collect(),
                line_index: LineIndex::new(input),
            });
        }

        let warnings: Vec<String> = warnings
            .iter()
            .map(|error| error.format_with_line_index(&LineIndex::new(input)))
            .collect();

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
            verify: true,
            format: None,
            touched: Vec::new(),
            newline,
            warnings,
        })
    }

    /// 読み飛ばした入力を報告する（現状は閉じないブロックコメント）。
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// 文字列のスタイルを指定する（既定は `Prefer::Preserve`）。
    pub fn string_style(mut self, style: Prefer<QuoteStyle>) -> Self {
        self.string_style = style;
        self
    }

    /// コメントのスタイルを指定する（既定は `Prefer::Preserve`）。
    pub fn comment_style(mut self, style: Prefer<CommentKind>) -> Self {
        self.comment_style = style;
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

    /// 編集結果の検証（再パース・値一致・コメント差分）を有効にするか。既定は有効。
    ///
    /// 検証に失敗した編集は結果を返さずエラーになる。入力長に比例する追加コストが
    /// かかるため、巨大な文書では無効化できる。
    pub fn verify_edits(mut self, verify: bool) -> Self {
        self.verify = verify;
        self
    }

    pub(crate) fn current_verify(&self) -> bool {
        self.verify
    }

    /// 部分整形（`format_container`）で使う整形設定。既定は文書から検出したもの。
    pub fn format_config(mut self, config: FormatConfig) -> Self {
        self.format = Some(config);
        self
    }

    /// 文書に設定された整形設定。
    pub fn current_format_config(&self) -> FormatConfig {
        self.format
            .clone()
            .unwrap_or_else(|| self.detected_format_config())
    }

    /// 編集で子が変わったコンテナ（外側優先・重複排除済み）。
    ///
    /// `format_container` に渡すことで、変更した箇所だけを整形できる。
    pub fn touched_containers(&self) -> &[HjsonObjectPath] {
        &self.touched
    }

    /// 変更したコンテナを記録する。祖先に覆われるものは残さない。
    pub(crate) fn note_touched(mut self, path: HjsonObjectPath) -> Self {
        let covered = self
            .touched
            .iter()
            .any(|existing| path.segments().starts_with(existing.segments()));
        if !covered {
            self.touched
                .retain(|existing| !existing.segments().starts_with(path.segments()));
            self.touched.push(path);
        }
        self
    }

    /// この文書で検出した改行コード。
    pub fn current_newline(&self) -> &str {
        &self.newline
    }

    /// lossless な構文木のルート。
    pub fn syntax_root(&self) -> &RedNode {
        &self.root
    }

    /// 意味ASTの文書を組み立てる。
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
