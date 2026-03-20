use rowan::{GreenNode, NodeOrToken};

use crate::{
    format::{Format, FormatConfig, Formatter},
    syntax::GreenElement,
    syntax::SyntaxKind,
};

pub(crate) struct TransparentFormatter;

impl Format for TransparentFormatter {
    fn format(&self, node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode {
        let mut new_children: Vec<GreenElement> = Vec::with_capacity(node.children().count());

        for child in node.children() {
            match child {
                NodeOrToken::Node(n) => {
                    new_children.push(
                        Formatter::format_with_depth(n.to_owned(), depth, config.clone()).into(),
                    );
                }
                NodeOrToken::Token(t) => {
                    new_children.push(t.to_owned().into());
                }
            }
        }

        SyntaxKind::Value.create_node(new_children)
    }
}
