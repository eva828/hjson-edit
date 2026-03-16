use rowan::GreenNode;

use crate::{format::Format, FormatConfig, Formatter};

pub(crate) struct RootFormatter;

impl Format for RootFormatter {
    fn format(&self, node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode {
        println!("[RootFormatter] Formatting value.");
        let Some(child) = node.children().next().and_then(|child| child.into_node()) else {
            return node;
        };
        Formatter::format_with_depth(child.to_owned(), depth, config)
    }
}
