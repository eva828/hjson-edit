use rowan::{GreenNode, GreenToken, Language, NodeOrToken};

use crate::{
    format::{Format, FormatConfig, Formatter},
    syntax::green::GreenElement,
    syntax::kind::{HjsonLanguage, SyntaxKind},
};

pub(crate) struct ArrayFormatter;

fn clone_element(
    elem: &rowan::NodeOrToken<&rowan::GreenNodeData, &rowan::GreenTokenData>,
) -> GreenElement {
    match elem {
        NodeOrToken::Node(n) => NodeOrToken::Node((*n).to_owned()),
        NodeOrToken::Token(t) => NodeOrToken::Token(GreenToken::new(t.kind(), t.text())),
    }
}

impl Format for ArrayFormatter {
    fn format(&self, node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode {
        println!("[ArrayFormatter] Formatting value.");
        node.children().for_each(|child| {
            println!("  Child: {:?}", child);
        });
        node
    }
}
