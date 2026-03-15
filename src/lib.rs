//! HJSON Parser and Editor
//!
//! 位置情報とコメントを保持したままHJSONを編集するためのライブラリ
//!
//! # Features
//! - HJSONフル対応（コメント、引用符なしキー/値、複数行文字列など）
//! - ロスレス編集（空白・改行・コメントを保持）
//! - Green/Red Tree方式による効率的な編集

pub(crate) mod document_style;
pub mod edit;
pub(crate) mod format;
pub(crate) mod number;
pub(crate) mod parser;
pub(crate) mod syntax;

// 公開APIを再エクスポート
pub use document_style::{DetectNewline, DocumentStyle, NewlineKind};
pub use edit::api::{EditError, GreenNodeEdit, RedNodeNavigateExt};
pub use edit::high_level::{
    CommentPath, CommentStyle, EditResult, FormatError, HjsonDocument, HjsonEditError, HjsonValue,
    IntoPath, PathSegment, StringStyle,
};
pub use format::FormatTarget;
pub use parser::parser::ParseError;
pub use syntax::green::GreenElement;
pub use syntax::kind::SyntaxKind;
pub use syntax::red::{RedNode, RedNodeCoreExt, RedToken};
