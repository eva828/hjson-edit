//! HJSON Parser and Editor
//!
//! 位置情報とコメントを保持したままHJSONを編集するためのライブラリ
//!
//! # Features
//! - HJSONフル対応（コメント、引用符なしキー/値、16進数、特殊浮動小数点など）
//! - ロスレス編集（空白・改行・コメントを保持）
//! - Green/Red Tree方式による効率的な編集
//! - 変更行の検出（1始まり）

pub mod syntax;
pub mod parser;
pub mod edit;
pub mod diff;

// 公開APIを再エクスポート
pub use syntax::kind::{HjsonLanguage, SyntaxKind};
pub use syntax::green::{GreenElement, GreenNodeExt, GreenTreeBuilder};
pub use syntax::red::{RedNode, RedToken, RedElement, RedNodeExt, RedElementExt};
pub use parser::lexer::{lex, LexResult, Token, LexError};
pub use parser::parser::{parse, ParseResult, ParseError};
pub use edit::api::{GreenNodeEdit, RedNodeEdit, EditOp, EditError, create_token, create_node};
pub use diff::{detect_changed_lines, simple_line_diff, ChangeInfo, ChangeType};

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
pub fn apply_edit(
    node: &rowan::GreenNode,
    edit: EditOp,
) -> Result<rowan::GreenNode, EditError> {
    match edit {
        EditOp::Replace { index, new } => {
            node.replace_child(index, new)
        }
        EditOp::Insert { index, child } => {
            node.insert_child(index, child)
        }
        EditOp::Delete { index } => {
            node.remove_child(index)
        }
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

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[test]
    fn test_full_workflow() {
        // HJSONをパース
        let input = r#"{
            // Configuration
            name: "app",
            port: 8080,
            debug: true
        }"#;
        
        let red = parse_to_red(input).unwrap();
        
        // GreenNodeに変換
        let green = red.to_green();
        
        // 文字列出力
        let output = green_to_string(&green);
        assert_eq!(output, input);
        
        // 変更行を検出（同じ内容なので変更なし）
        let changed = get_changed_lines(&green, &green, input);
        assert!(changed.is_empty());
    }
    
    #[test]
    fn test_edit_and_detect_changes() {
        let input = r#"{"key": "value"}"#;

        let red = parse_to_red(input).unwrap();
        let green_before = red.to_green();

        // 構造: Root > Value > Object > Pair > Value
        let root_value = red.child_by_kind(SyntaxKind::Value).expect("Root Value not found");
        let object = root_value.child_by_kind(SyntaxKind::Object).expect("Object not found");
        let pair = object.children_nodes().next().expect("Pair not found");
        let pair_value = pair.child_by_kind(SyntaxKind::Value).expect("Pair Value not found");

        // 新しいトークンを作成
        let new_token = create_token(SyntaxKind::String, "\"new_value\"");

        // 子インデックスを見つける
        let value_green = pair_value.to_green();
        let mut target_idx = None;
        for (i, child) in value_green.children().enumerate() {
            if let rowan::NodeOrToken::Token(t) = child {
                if t.text() == "\"value\"" {
                    target_idx = Some(i);
                    break;
                }
            }
        }

        let target_idx = target_idx.expect("Target token not found");
        let new_value_green = value_green.replace_child(target_idx, new_token.into()).unwrap();

        // 変更行を検出
        let changed = get_changed_lines(&green_before, &new_value_green, input);

        // 1行目が変更されたはず
        assert_eq!(changed, vec![1]);
    }
    
    #[test]
    fn test_special_values() {
        let input = r#"{
            hex: 0xFF,
            nan_val: NaN,
            inf_val: Infinity,
            neg_inf: -Infinity
        }"#;
        
        let result = parse(input);
        assert!(result.is_ok());
        
        let red = parse_to_red(input).unwrap();
        let output = red_to_string(&red);
        assert_eq!(output, input);
    }
    
    #[test]
    fn test_comments_preserved() {
        let input = r#"{
            // Line comment
            # Hash comment
            /* Block
               comment */
            key: value
        }"#;
        
        let red = parse_to_red(input).unwrap();
        let output = red_to_string(&red);
        assert_eq!(output, input);
    }
    
    #[test]
    fn test_block_string() {
        let input = "'''multi\nline\ntext'''";
        
        let red = parse_to_red(input).unwrap();
        let output = red_to_string(&red);
        assert_eq!(output, input);
    }
    
    #[test]
    fn test_error_handling() {
        let input = "{invalid";
        
        let result = parse(input);
        assert!(result.is_err());
    }
}
