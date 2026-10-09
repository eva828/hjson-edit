use line_index::LineIndex;

use crate::{high_level::path::HjsonObjectPath, syntax::lexer::LexError, ParseError, SyntaxKind};

/// エラー表示に含めるパスの最大セグメント数と、入力・パスの最大文字数。
/// 過大な引数でエラー出力が増幅しないようにする。
const MAX_ERROR_PATH_SEGMENTS: usize = 8;
const MAX_ERROR_TEXT_CHARS: usize = 120;

#[derive(Debug, Clone, thiserror::Error)]
pub enum HjsonEditError {
    #[error("Path not found: {}", format_path(.0))]
    PathNotFound(HjsonObjectPath),
    #[error("Invalid path: {}", format_text(.0))]
    InvalidPath(String),
    #[error("Unexpected structure: expected {expected:?}, found {found:?}")]
    UnexpectedStructure {
        expected: SyntaxKind,
        found: SyntaxKind,
    },
    #[error("Parent node not found")]
    ParentNotFound,
    #[error("Parse errors:\n{}", format_parse_errors(errors, line_index))]
    ParseError {
        errors: Vec<ParseError>,
        line_index: LineIndex,
    },
    #[error("Lex errors:\n{}", format_lex_errors(errors, line_index))]
    LexError {
        errors: Vec<LexError>,
        line_index: LineIndex,
    },
    #[error("Array index out of bounds: index {index}, length {len}")]
    ArrayIndexOutOfBounds { index: usize, len: usize },
    #[error("Cannot merge: {0}")]
    MergeConflict(String),
    #[error("Input too long: {len} bytes, limit is {limit} bytes")]
    InputTooLong { len: usize, limit: usize },
    #[error("Nesting too deep: depth {depth}, limit is {limit}")]
    NestingTooDeep { depth: usize, limit: usize },
}

/// パスを、先頭のみを残して省略した表示にする。
fn format_path(path: &HjsonObjectPath) -> String {
    let segments = path.segments();
    if segments.len() <= MAX_ERROR_PATH_SEGMENTS {
        return format!("{segments:?}");
    }

    let head = &segments[..MAX_ERROR_PATH_SEGMENTS];
    format!("{head:?} … ({} segments)", segments.len())
}

/// 過大な入力を、先頭のみを残して省略した表示にする。
fn format_text(text: &str) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(MAX_ERROR_TEXT_CHARS).collect();
    let head_len = head.chars().count();
    let total = head_len + chars.count();
    if total == head_len {
        return format!("{head:?}");
    }

    format!("{head:?} … ({total} chars)")
}

fn format_parse_errors(errors: &[ParseError], line_index: &LineIndex) -> String {
    errors
        .iter()
        .map(|e| e.format_with_line_index(line_index))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_lex_errors(errors: &[LexError], line_index: &LineIndex) -> String {
    errors
        .iter()
        .map(|e| e.format_with_line_index(line_index))
        .collect::<Vec<_>>()
        .join("\n")
}
