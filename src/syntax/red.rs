use super::kind::{HjsonLanguage, SyntaxKind};
use rowan::{SyntaxNode, SyntaxToken};

/// RedNode型エイリアス
pub type RedNode = SyntaxNode<HjsonLanguage>;
/// RedToken型エイリアス
pub type RedToken = SyntaxToken<HjsonLanguage>;
/// RedElement型エイリアス
pub type RedElement = rowan::SyntaxElement<HjsonLanguage>;

/// RedNodeユーティリティ拡張
pub trait RedNodeExt {
    /// 指定されたkindの子ノードを探す
    fn child_by_kind(&self, kind: SyntaxKind) -> Option<RedNode>;

    /// 指定されたkindの子トークンを探す
    fn token_by_kind(&self, kind: SyntaxKind) -> Option<RedToken>;

    /// すべての子ノードを取得
    fn children_nodes(&self) -> impl Iterator<Item = RedNode>;

    /// テキスト表現（フォーマット保持）
    fn text(&self) -> String;

    /// このノードに含まれる行番号範囲を取得（1始まり）
    fn line_range(&self, source: &str) -> (usize, usize);

    /// 子をインデックスで取得
    fn child_at(&self, index: usize) -> Option<RedElement>;

    /// 子の数を取得
    fn child_count(&self) -> usize;
}

impl RedNodeExt for RedNode {
    fn child_by_kind(&self, kind: SyntaxKind) -> Option<RedNode> {
        // children() returns Iterator<Item = SyntaxNode>, not NodeOrToken
        self.children().find(|child| child.kind() == kind)
    }

    fn token_by_kind(&self, kind: SyntaxKind) -> Option<RedToken> {
        self.children_with_tokens().find_map(|child| {
            if let rowan::NodeOrToken::Token(token) = child {
                if token.kind() == kind {
                    return Some(token);
                }
            }
            None
        })
    }

    fn children_nodes(&self) -> impl Iterator<Item = RedNode> {
        // children() already returns Iterator<Item = SyntaxNode>
        self.children()
    }

    fn text(&self) -> String {
        self.to_string()
    }

    fn line_range(&self, source: &str) -> (usize, usize) {
        let offset = self.text_range().start().into();
        let text_before = &source[..offset];
        let start_line = text_before.chars().filter(|&c| c == '\n').count() + 1;

        // Convert SyntaxText to String first, then count newlines
        let node_text = self.to_string();
        let newlines_in_node = node_text.chars().filter(|&c| c == '\n').count();
        let end_line = start_line + newlines_in_node;

        (start_line, end_line)
    }

    fn child_at(&self, index: usize) -> Option<RedElement> {
        self.children_with_tokens().nth(index)
    }

    fn child_count(&self) -> usize {
        self.children_with_tokens().count()
    }
}

/// RedNodeを操作する際のヘルパー
pub trait RedElementExt {
    fn as_node(&self) -> Option<&RedNode>;
    fn as_token(&self) -> Option<&RedToken>;
    fn text(&self) -> String;
}

impl RedElementExt for RedElement {
    fn as_node(&self) -> Option<&RedNode> {
        match self {
            rowan::NodeOrToken::Node(node) => Some(node),
            rowan::NodeOrToken::Token(_) => None,
        }
    }

    fn as_token(&self) -> Option<&RedToken> {
        match self {
            rowan::NodeOrToken::Node(_) => None,
            rowan::NodeOrToken::Token(token) => Some(token),
        }
    }

    fn text(&self) -> String {
        match self {
            rowan::NodeOrToken::Node(node) => node.to_string(),
            rowan::NodeOrToken::Token(token) => token.text().to_string(),
        }
    }
}

/// RedNodeからGreenNodeを再構築するヘルパー
pub fn red_to_green(node: &RedNode) -> rowan::GreenNode {
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
    build_recursive(&mut builder, &rowan::NodeOrToken::Node(node.clone()));
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::super::green::GreenTreeBuilder;
    use super::*;

    fn create_test_node() -> RedNode {
        let mut builder = GreenTreeBuilder::new();
        builder.start_node(SyntaxKind::Root);
        builder.start_node(SyntaxKind::Object);
        builder.token(SyntaxKind::LBrace, "{");
        builder.finish_node();
        builder.finish_node();

        let green = builder.finish();
        RedNode::new_root(green)
    }

    /// child_by_kindで指定したSyntaxKindの子ノードが取得できることを検証
    #[test]
    fn test_child_by_kind() {
        let node = create_test_node();
        let object = node.child_by_kind(SyntaxKind::Object);
        assert!(object.is_some());
    }

    /// line_rangeが1行のHJSON入力に対して (1, 1) を返すことを検証
    #[test]
    fn test_line_range() {
        // 実際のパース結果を使ってテスト
        let input = "{ \"key\": \"value\" }";
        let result = crate::parser::parser::parse(input).unwrap();
        let node = RedNode::new_root(result.green);

        // ルートノードは1行目
        let (start, end) = node.line_range(input);
        assert_eq!(start, 1);
        assert_eq!(end, 1);
    }
}
