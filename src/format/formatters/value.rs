// use crate::edit::high_level::{
//     create_multiline_string_literal, parse_multiline_string, FormatError,
// };
// use crate::format::{Format, FormatConfig};
// use crate::syntax::kind::SyntaxKind;
// use crate::syntax::red::RedNode;
// use rowan::{GreenNode, NodeOrToken};

use rowan::GreenNode;

use crate::{format::Format, FormatConfig, Formatter};

pub(crate) struct ValueFormatter;

impl Format for ValueFormatter {
    fn format(&self, node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode {
        println!("[ValueFormatter] Formatting value.");
        let Some(child) = node.children().next().and_then(|child| child.into_node()) else {
            return node;
        };
        Formatter::format_with_depth(child.to_owned(), depth, config)
    }
}
