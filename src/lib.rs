//! HJSON Parser and Editor
//!
//! 位置情報とコメントを保持したままHJSONを編集するためのライブラリ
//!
//! # Features
//! - HJSONフル対応（コメント、引用符なしキー/値、複数行文字列など）
//! - ロスレス編集（空白・改行・コメントを保持）
//! - Green/Red Tree方式による効率的な編集
//! - 変更行の検出（1始まり）

pub mod diff;
pub mod edit;
pub mod format;
pub mod parser;
pub mod syntax;

// 公開APIを再エクスポート
pub use diff::{detect_changed_lines, simple_line_diff, ChangeInfo, ChangeType};
pub use edit::api::{create_node, create_token, EditError, EditOp, GreenNodeEdit, RedNodeEdit};
pub use edit::high_level::{
    CommentPath, CommentStyle, EditResult, FormatError, HjsonDocument, HjsonEditError, HjsonValue,
    IntoPath, PathSegment, StringStyle,
};
pub use format::FormatTarget;
pub use parser::lexer::{lex, LexError, LexResult, Token};
pub use parser::parser::{parse, ParseError, ParseResult};
pub use syntax::green::{GreenElement, GreenNodeExt, GreenTreeBuilder};
pub use syntax::kind::{HjsonLanguage, SyntaxKind};
pub use syntax::red::{RedElement, RedElementExt, RedNode, RedNodeExt, RedToken};

/// パースしてRedNodeを取得（エラー時はErr）
pub fn parse_to_red(input: &str) -> Result<RedNode, Vec<ParseError>> {
    let result = parse(input)?;
    Ok(RedNode::new_root(result.green))
}

/// GreenNodeを文字列に変換（フォーマット保持）
pub fn green_to_string(node: &rowan::GreenNode) -> String {
    use syntax::green::GreenNodeExt;
    node.text()
}

/// RedNodeを文字列に変換（フォーマット保持）
pub fn red_to_string(node: &RedNode) -> String {
    node.to_string()
}

/// 編集操作を適用して新しいGreenNodeを取得
pub fn apply_edit(node: &rowan::GreenNode, edit: EditOp) -> Result<rowan::GreenNode, EditError> {
    match edit {
        EditOp::Replace { index, new } => node.replace_child(index, new),
        EditOp::Insert { index, child } => node.insert_child(index, child),
        EditOp::Delete { index } => node.remove_child(index),
    }
}

/// 複数の編集を適用
pub fn apply_edits(
    node: &rowan::GreenNode,
    edits: Vec<EditOp>,
) -> Result<rowan::GreenNode, EditError> {
    node.apply_edits(edits)
}

/// 変更行を検出
pub fn get_changed_lines(
    before: &rowan::GreenNode,
    after: &rowan::GreenNode,
    source: &str,
) -> Vec<usize> {
    detect_changed_lines(before, after, source)
}
