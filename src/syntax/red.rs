use super::kind::{HjsonLanguage, SyntaxKind};
use rowan::{SyntaxNode, SyntaxToken};

/// RedNode型エイリアス
pub type RedNode = SyntaxNode<HjsonLanguage>;
/// RedToken型エイリアス
pub type RedToken = SyntaxToken<HjsonLanguage>;
/// RedElement型エイリアス
pub type RedElement = rowan::SyntaxElement<HjsonLanguage>;

/// RedNodeの基本ユーティリティ拡張
pub trait RedNodeCoreExt {
    /// 指定されたkindの子ノードを探す
    fn child_by_kind(&self, kind: SyntaxKind) -> Option<RedNode>;

    /// GreenNodeを再構築する
    fn to_green(&self) -> rowan::GreenNode;
}

impl RedNodeCoreExt for RedNode {
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
