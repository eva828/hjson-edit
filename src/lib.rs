//! HJSON Parser and Editor
//!
//! 位置情報とコメントを保持したままHJSONを編集するためのライブラリ
//!
//! # Features
//! - HJSONフル対応（コメント、引用符なしキー/値、複数行文字列など）
//! - ロスレス編集（空白・改行・コメントを保持）
//! - Green/Red Tree方式による効率的な編集

pub mod ast;
pub(crate) mod format;
pub mod high_level;
pub(crate) mod number;
pub(crate) mod syntax;

// 公開APIを再エクスポート
pub use format::{Format, FormatConfig, IndentStyle};
pub use high_level::{
    error::HjsonEditError,
    ops::merge::HjsonDocumentMerge,
    ops::object::{AppendMode, HjsonDocumentObjectEdit},
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
