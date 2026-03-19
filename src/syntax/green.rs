/// GreenElement型エイリアス（NodeまたはToken）
pub type GreenElement = rowan::NodeOrToken<rowan::GreenNode, rowan::GreenToken>;

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
