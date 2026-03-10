/// 低レベル編集API
///
/// Green/Red Treeを操作するための基本的な編集機能
use super::super::syntax::green::GreenElement;
use super::super::syntax::kind::SyntaxKind;
use super::super::syntax::red::RedNode;
use rowan::GreenNodeBuilder;

/// 編集操作
#[derive(Debug, Clone)]
pub enum EditOp {
    /// 子を置換
    Replace { index: usize, new: GreenElement },
    /// 子を挿入
    Insert { index: usize, child: GreenElement },
    /// 子を削除
    Delete { index: usize },
}

/// 編集エラー
#[derive(Debug, Clone, thiserror::Error)]
pub enum EditError {
    #[error("Invalid index: {index} (child count: {count})")]
    InvalidIndex { index: usize, count: usize },
    #[error("Expected {expected:?}, found {found:?}")]
    InvalidNodeKind {
        expected: SyntaxKind,
        found: SyntaxKind,
    },
}

/// GreenNodeに対する編集操作
pub trait GreenNodeEdit {
    /// 指定インデックスの子を置換
    fn replace_child(
        &self,
        index: usize,
        replacement: GreenElement,
    ) -> Result<rowan::GreenNode, EditError>;

    /// 指定インデックスに子を挿入
    fn insert_child(
        &self,
        index: usize,
        child: GreenElement,
    ) -> Result<rowan::GreenNode, EditError>;

    /// 指定インデックスの子を削除
    fn remove_child(&self, index: usize) -> Result<rowan::GreenNode, EditError>;

    /// 複数の編集を適用
    fn apply_edits(&self, edits: Vec<EditOp>) -> Result<rowan::GreenNode, EditError>;
}

impl GreenNodeEdit for rowan::GreenNode {
    fn replace_child(
        &self,
        index: usize,
        replacement: GreenElement,
    ) -> Result<rowan::GreenNode, EditError> {
        let children: Vec<GreenElement> = self.children().map(child_to_green_element).collect();

        if index >= children.len() {
            return Err(EditError::InvalidIndex {
                index,
                count: children.len(),
            });
        }

        let mut builder = GreenNodeBuilder::new();
        builder.start_node(self.kind());

        for (i, child) in children.iter().enumerate() {
            if i == index {
                push_element(&mut builder, &replacement);
            } else {
                push_element(&mut builder, child);
            }
        }

        builder.finish_node();
        Ok(builder.finish())
    }

    fn insert_child(
        &self,
        index: usize,
        child: GreenElement,
    ) -> Result<rowan::GreenNode, EditError> {
        let children: Vec<GreenElement> = self.children().map(child_to_green_element).collect();

        if index > children.len() {
            return Err(EditError::InvalidIndex {
                index,
                count: children.len(),
            });
        }

        let mut builder = GreenNodeBuilder::new();
        builder.start_node(self.kind());

        for (i, c) in children.iter().enumerate() {
            if i == index {
                push_element(&mut builder, &child);
            }
            push_element(&mut builder, c);
        }

        // 末尾挿入の場合
        if index == children.len() {
            push_element(&mut builder, &child);
        }

        builder.finish_node();
        Ok(builder.finish())
    }

    fn remove_child(&self, index: usize) -> Result<rowan::GreenNode, EditError> {
        let children: Vec<GreenElement> = self.children().map(child_to_green_element).collect();

        if index >= children.len() {
            return Err(EditError::InvalidIndex {
                index,
                count: children.len(),
            });
        }

        let mut builder = GreenNodeBuilder::new();
        builder.start_node(self.kind());

        for (i, child) in children.iter().enumerate() {
            if i != index {
                push_element(&mut builder, child);
            }
        }

        builder.finish_node();
        Ok(builder.finish())
    }

    fn apply_edits(&self, edits: Vec<EditOp>) -> Result<rowan::GreenNode, EditError> {
        // インデックスを計算してソート
        let mut indexed_ops: Vec<(usize, EditOp)> = edits
            .into_iter()
            .map(|op| {
                let idx = match &op {
                    EditOp::Replace { index, .. } => *index,
                    EditOp::Insert { index, .. } => *index,
                    EditOp::Delete { index } => *index,
                };
                (idx, op)
            })
            .collect();

        // インデックス降順でソート（後ろから処理）
        indexed_ops.sort_by_key(|item| std::cmp::Reverse(item.0));

        // 重複チェック
        let mut seen = std::collections::HashSet::new();
        for (idx, _) in &indexed_ops {
            if !seen.insert(*idx) {
                return Err(EditError::InvalidIndex {
                    index: *idx,
                    count: 0,
                });
            }
        }

        let mut result = self.clone();

        for (_, op) in indexed_ops {
            result = match op {
                EditOp::Replace { index, new } => result.replace_child(index, new)?,
                EditOp::Insert { index, child } => result.insert_child(index, child)?,
                EditOp::Delete { index } => result.remove_child(index)?,
            };
        }

        Ok(result)
    }
}

/// RedNodeに対する編集操作
pub trait RedNodeEdit {
    /// このノードを含む新しいGreenNodeを作成
    fn to_green(&self) -> rowan::GreenNode;

    /// 親をたどってパスを取得
    fn path_from_root(&self) -> Vec<usize>;

    /// ルートから見た子インデックスを計算
    fn child_index_in_parent(&self) -> Option<usize>;
}

impl RedNodeEdit for RedNode {
    fn to_green(&self) -> rowan::GreenNode {
        super::super::syntax::red::red_to_green(self)
    }

    fn path_from_root(&self) -> Vec<usize> {
        let mut path = Vec::new();
        let mut current = self.clone();

        while let Some(parent) = current.parent() {
            if let Some(idx) = current.child_index_in_parent() {
                path.push(idx);
            }
            current = parent;
        }

        path.reverse();
        path
    }

    fn child_index_in_parent(&self) -> Option<usize> {
        Some(self.index())
    }
}

/// 子要素をGreenElementに変換
fn child_to_green_element(
    child: rowan::NodeOrToken<&rowan::GreenNodeData, &rowan::GreenTokenData>,
) -> GreenElement {
    match child {
        rowan::NodeOrToken::Node(node_data) => rowan::NodeOrToken::Node(rowan::GreenNode::from(
            std::borrow::Cow::Borrowed(node_data),
        )),
        rowan::NodeOrToken::Token(token_data) => {
            rowan::NodeOrToken::Token(rowan::GreenToken::new(token_data.kind(), token_data.text()))
        }
    }
}

/// GreenElementをBuilderに追加
fn push_element(builder: &mut GreenNodeBuilder<'static>, element: &GreenElement) {
    match element {
        rowan::NodeOrToken::Node(node) => {
            builder.start_node(node.kind());
            for child in node.children() {
                let elem = child_to_green_element(child);
                push_element(builder, &elem);
            }
            builder.finish_node();
        }
        rowan::NodeOrToken::Token(token) => {
            let kind: rowan::SyntaxKind = token.kind();
            builder.token(kind, token.text());
        }
    }
}

/// ヘルパー関数：新しいトークンを作成
pub fn create_token(kind: SyntaxKind, text: impl Into<String>) -> rowan::GreenToken {
    rowan::GreenToken::new(kind.into(), text.into().as_str())
}

/// ヘルパー関数：新しいノードを作成
pub fn create_node(kind: SyntaxKind, children: Vec<GreenElement>) -> rowan::GreenNode {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(kind.into());
    for child in children {
        push_element(&mut builder, &child);
    }
    builder.finish_node();
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::super::super::syntax::green::GreenNodeExt;
    use super::super::super::syntax::green::GreenTreeBuilder;
    use super::*;

    fn create_test_node() -> rowan::GreenNode {
        let mut builder = GreenTreeBuilder::new();
        builder.start_node(SyntaxKind::Object);
        builder.token(SyntaxKind::LBrace, "{");
        builder.token(SyntaxKind::RBrace, "}");
        builder.finish_node();
        builder.finish()
    }

    /// replace_childで子トークンを置換できることを検証
    ///
    /// LBrace "{" を LBracket "[" に置換し、結果が "[}" になることを確認する
    #[test]
    fn test_replace_child() {
        let node = create_test_node();
        let new_token = create_token(SyntaxKind::LBracket, "[");

        let result = node.replace_child(0, new_token.into()).unwrap();
        let result_ref: &rowan::GreenNode = &result;
        assert_eq!(result_ref.text(), "[}");
    }

    /// insert_childで指定位置に新しいトークンを挿入できることを検証
    ///
    /// "{}" のインデックス1にWhitespaceを挿入し、"{ }" になることを確認する
    #[test]
    fn test_insert_child() {
        let node = create_test_node();
        let new_token = create_token(SyntaxKind::Whitespace, " ");

        let result = node.insert_child(1, new_token.into()).unwrap();
        let result_ref: &rowan::GreenNode = &result;
        assert_eq!(result_ref.text(), "{ }");
    }

    /// remove_childで指定位置の子トークンを削除できることを検証
    ///
    /// "{}" のインデックス1（RBrace）を削除し、"{" になることを確認する
    #[test]
    fn test_remove_child() {
        let node = create_test_node();

        let result = node.remove_child(1).unwrap();
        let result_ref: &rowan::GreenNode = &result;
        assert_eq!(result_ref.text(), "{");
    }

    /// 範囲外のインデックスを指定するとEditError::InvalidIndexが返ることを検証
    #[test]
    fn test_invalid_index() {
        let node = create_test_node();
        let new_token = create_token(SyntaxKind::LBrace, "{");

        let result = node.replace_child(10, new_token.into());
        assert!(matches!(result, Err(EditError::InvalidIndex { .. })));
    }
}
