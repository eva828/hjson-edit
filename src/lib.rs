#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod ast;
#[doc(hidden)]
pub mod conformance;
pub(crate) mod format;
pub mod high_level;
pub(crate) mod number;
pub(crate) mod syntax;

// 公開APIを再エクスポート
pub use format::{Format, FormatConfig, IndentStyle};
pub use high_level::{
    error::HjsonEditError,
    ops::merge::HjsonDocumentMerge,
    ops::object::HjsonDocumentEdit,
    path::{ArrayPathSegment, HjsonObjectPath, PathSegment},
    value::HjsonValue,
    HjsonDocument, Prefer,
};
pub use line_index::LineIndex;
pub use syntax::parser::{ParseError, MAX_NESTING_DEPTH};
pub use syntax::GreenElement;
pub use syntax::SyntaxKind;
pub use syntax::MAX_INPUT_BYTES;
pub use syntax::{RedNode, RedToken};
