use line_index::LineIndex;

use crate::{
    high_level::value::HjsonValue, syntax::lexer::LexError, HjsonPath, ParseError, SyntaxKind,
};

#[derive(Debug, Clone, thiserror::Error)]
pub enum HjsonEditError {
    #[error("Path not found: {0:?}")]
    PathNotFound(HjsonPath),
    #[error("Invalid path: {0:?}")]
    InvalidPath(String),
    #[error("Unexpected structure: expected {expected:?}, found {found:?}")]
    UnexpectedStructure {
        expected: SyntaxKind,
        found: SyntaxKind,
    },
    #[error("Parent node not found")]
    ParentNotFound,
    #[error("Type mismatch: expected {expected:?}, found {found:?}")]
    TypeMismatch {
        expected: HjsonValue,
        found: HjsonValue,
    },
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
