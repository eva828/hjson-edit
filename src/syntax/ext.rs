use super::{RedElement, RedNode, SyntaxKind};
use crate::high_level::error::HjsonEditError;
use crate::high_level::path::ArrayPathSegment;

/// GreenNodeユーティリティ拡張
pub trait GreenNodeExt {
    /// テキスト表現を取得（フォーマット保持）
    fn text(&self) -> String;
}

impl GreenNodeExt for rowan::GreenNode {
    fn text(&self) -> String {
        fn node_text_recursive(node: &rowan::GreenNode) -> String {
            let mut result = String::new();
            for child in node.children() {
                match child {
                    rowan::NodeOrToken::Node(n) => {
                        // n は &GreenNodeData
                        // cloneでArcを取得してGreenNodeを作成
                        let green_node: rowan::GreenNode =
                            rowan::GreenNode::from(std::borrow::Cow::Borrowed(n));
                        result.push_str(&node_text_recursive(&green_node));
                    }
                    rowan::NodeOrToken::Token(t) => {
                        result.push_str(t.text());
                    }
                }
            }
            result
        }
        node_text_recursive(self)
    }
}

/// RedNodeの基本ユーティリティ拡張
pub trait RedNodeExt {
    /// 指定されたkindの子ノードを探す
    fn child_by_kind(&self, kind: SyntaxKind) -> Option<RedNode>;

    /// GreenNodeを再構築する
    fn to_green(&self) -> rowan::GreenNode;

    /// Object内でキー検索
    fn find_pair_by_key(&self, key: &str) -> Option<RedNode>;

    /// Array内で要素取得
    fn get_array_element(&self, op: &ArrayPathSegment) -> Result<Option<RedNode>, HjsonEditError>;
}

impl RedNodeExt for RedNode {
    fn child_by_kind(&self, kind: SyntaxKind) -> Option<RedNode> {
        // children() returns Iterator<Item = SyntaxNode>, not NodeOrToken
        self.children().find(|child| child.kind() == kind)
    }

    fn to_green(&self) -> rowan::GreenNode {
        use rowan::GreenNodeBuilder;

        fn build_recursive(builder: &mut GreenNodeBuilder<'static>, element: &RedElement) {
            match element {
                rowan::NodeOrToken::Node(node) => {
                    builder.start_node(node.kind().into());
                    for child in node.children_with_tokens() {
                        build_recursive(builder, &child);
                    }
                    builder.finish_node();
                }
                rowan::NodeOrToken::Token(token) => {
                    builder.token(token.kind().into(), token.text());
                }
            }
        }

        let mut builder = GreenNodeBuilder::new();
        build_recursive(&mut builder, &rowan::NodeOrToken::Node(self.clone()));
        builder.finish()
    }

    fn find_pair_by_key(&self, key: &str) -> Option<RedNode> {
        if self.kind() != SyntaxKind::Object {
            return None;
        }

        self.children()
            .filter(|child| child.kind() == SyntaxKind::Pair)
            .find(|pair| {
                pair.children()
                    .find(|child| child.kind() == SyntaxKind::Key)
                    .and_then(|key_node| {
                        key_node.children_with_tokens().find_map(|elem| {
                            if let rowan::NodeOrToken::Token(token) = elem {
                                if matches!(
                                    token.kind(),
                                    SyntaxKind::String
                                        | SyntaxKind::UnquotedString
                                        | SyntaxKind::BlockString
                                ) {
                                    return Some(token.text().trim_matches('"').to_string());
                                }
                            }
                            None
                        })
                    })
                    .map(|k| k == key)
                    .unwrap_or(false)
            })
    }

    fn get_array_element(&self, op: &ArrayPathSegment) -> Result<Option<RedNode>, HjsonEditError> {
        if self.kind() != SyntaxKind::Array {
            return Ok(None);
        }

        let elements: Vec<RedNode> = self
            .children()
            .filter(|child| child.kind() == SyntaxKind::Value)
            .collect();

        match op {
            ArrayPathSegment::Index(idx) => {
                if *idx >= elements.len() {
                    Err(HjsonEditError::ChildNotFound(vec![]))
                } else {
                    Ok(elements.into_iter().nth(*idx))
                }
            }
            ArrayPathSegment::InsertAt(idx) => {
                // Return position info for insertion
                if *idx > elements.len() {
                    Err(HjsonEditError::ChildNotFound(vec![]))
                } else if *idx == elements.len() {
                    // Insert at end (push)
                    Ok(None)
                } else {
                    // Return the element at this index (will insert before it)
                    Ok(elements.into_iter().nth(*idx))
                }
            }
            ArrayPathSegment::Push => {
                // Always insert at end
                Ok(None)
            }
        }
    }
}
