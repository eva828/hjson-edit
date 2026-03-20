use super::{RedElement, RedNode, SyntaxKind};

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
}
