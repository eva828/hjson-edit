use line_index::LineIndex;
use std::sync::Arc;

use crate::{high_level::path::HjsonObjectPath, syntax::lexer::LexError, ParseError, SyntaxKind};

/// エラー表示に含めるパスの最大セグメント数と、入力・パスの最大文字数。
/// 過大な引数でエラー出力が増幅しないようにする。
const MAX_ERROR_PATH_SEGMENTS: usize = 8;
const MAX_ERROR_TEXT_CHARS: usize = 120;

/// パース・編集で起きるエラー。
#[derive(Debug, Clone, thiserror::Error)]
pub enum HjsonEditError {
    /// パスが指す値が無い。
    #[error("Path not found: {}", format_path(.0))]
    PathNotFound(HjsonObjectPath),
    /// パスの書式が不正。
    #[error("Invalid path: {}", format_plain(.0))]
    InvalidPath(String),
    /// 期待した構造と違う。
    #[error("Unexpected structure: expected {expected:?}, found {found:?}")]
    UnexpectedStructure {
        /// 期待した種別。
        expected: SyntaxKind,
        /// 実際の種別。
        found: SyntaxKind,
    },
    /// 操作の対象として不正。
    #[error("Invalid target {}: {}", format_path(.path), format_text(.reason))]
    InvalidTarget {
        /// 対象のパス。
        path: HjsonObjectPath,
        /// 理由。
        reason: String,
    },
    /// 編集結果が検証を通らなかった。
    #[error("Edit produced an invalid document at {}: {}", format_text(.target), format_text(.detail))]
    InvalidEdit {
        /// 対象（パスまたはコメント位置）。
        target: String,
        /// 検出した差分。
        detail: String,
    },
    /// パースエラー。
    #[error("Parse errors:\n{}", format_parse_errors(errors, line_index, input))]
    ParseError {
        /// 検出したエラー。
        errors: Vec<ParseError>,
        /// 行・列の計算に使う索引。
        line_index: LineIndex,
        /// 行・列の計算に使う入力（列は文字数で数える）。
        input: Arc<str>,
    },
    /// 字句エラー。
    #[error("Lex errors:\n{}", format_lex_errors(errors, line_index, input))]
    LexError {
        /// 検出したエラー。
        errors: Vec<LexError>,
        /// 行・列の計算に使う索引。
        line_index: LineIndex,
        /// 行・列の計算に使う入力（列は文字数で数える）。
        input: Arc<str>,
    },
    /// 配列の index が範囲外。
    #[error("Array index out of bounds: index {index}, length {len}")]
    ArrayIndexOutOfBounds {
        /// 指定された index。
        index: usize,
        /// 配列の長さ。
        len: usize,
    },
    /// マージできない。
    #[error("Cannot merge: {0}")]
    MergeConflict(String),
    /// 入力が長すぎる。
    #[error("Input too long: {len} bytes, limit is {limit} bytes")]
    InputTooLong {
        /// 入力の長さ。
        len: usize,
        /// 上限。
        limit: usize,
    },
    /// ネストが深すぎる。
    #[error("Nesting too deep: depth {depth}, limit is {limit}")]
    NestingTooDeep {
        /// 実際の深さ。
        depth: usize,
        /// 上限。
        limit: usize,
    },
    /// その値を HJSON では書けない。
    #[error("Value cannot be represented in HJSON: {reason}")]
    ValueNotRepresentable {
        /// 理由。
        reason: String,
    },
}

/// パスを、先頭のみを残して省略した表示にする。
fn format_path(path: &HjsonObjectPath) -> String {
    let segments = path.segments();
    if segments.len() <= MAX_ERROR_PATH_SEGMENTS {
        return path.to_string();
    }

    let head = HjsonObjectPath::from_segments(segments[..MAX_ERROR_PATH_SEGMENTS].to_vec());
    format!("{head} … ({} segments)", segments.len())
}

/// 利用者が書いた文字列を、引用符を付けずに省略して表示する。
fn format_plain(text: &str) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(MAX_ERROR_TEXT_CHARS).collect();
    let head_len = head.chars().count();
    let total = head_len + chars.count();
    if total == head_len {
        return head;
    }

    format!("{head} … ({total} chars)")
}

/// 過大な入力を、先頭のみを残して省略した表示にする。
pub(crate) fn format_text(text: &str) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(MAX_ERROR_TEXT_CHARS).collect();
    let head_len = head.chars().count();
    let total = head_len + chars.count();
    if total == head_len {
        return format!("{head:?}");
    }

    format!("{head:?} … ({total} chars)")
}

fn format_parse_errors(errors: &[ParseError], line_index: &LineIndex, input: &str) -> String {
    errors
        .iter()
        .map(|e| e.format_with_source(line_index, input))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_lex_errors(errors: &[LexError], line_index: &LineIndex, input: &str) -> String {
    errors
        .iter()
        .map(|e| e.format_with_source(line_index, input))
        .collect::<Vec<_>>()
        .join("\n")
}
