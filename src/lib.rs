//! HJSON Parser and Editor
//!
//! 位置情報とコメントを保持したままHJSONを編集するためのライブラリ
//!
//! # Features
//! - HJSONフル対応（コメント、引用符なしキー/値、複数行文字列など）
//! - ロスレス編集（空白・改行・コメントを保持）
//! - Green/Red Tree方式による効率的な編集

pub(crate) mod document_style;
pub(crate) mod format;
pub mod high_level;
pub(crate) mod number;
pub(crate) mod syntax;

// 公開APIを再エクスポート
pub use document_style::{DetectNewline, DocumentStyle, NewlineKind};
pub use format::{FormatConfig, Formatter};
pub use high_level::{
    CommentPath, CommentStyle, DocumentArrayEdit, DocumentComment, DocumentFormat,
    DocumentValueEdit, EditResult, FormatError, FormatTarget, HjsonDocument, HjsonEditError,
    HjsonValue, IntoPath, PathSegment, StringStyle,
};
pub use line_index::LineIndex;
pub use syntax::ext::RedNodeExt;
pub use syntax::parser::ParseError;
pub use syntax::GreenElement;
pub use syntax::SyntaxKind;
pub use syntax::{RedNode, RedToken};
