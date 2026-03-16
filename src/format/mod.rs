use rowan::{GreenNode, Language};

use crate::{
    format::formatters::{
        ArrayFormatter, ObjectFormatter, PairFormatter, RootFormatter, ValueFormatter,
    },
    syntax::kind::HjsonLanguage,
    SyntaxKind,
};

pub(crate) mod formatters;

#[derive(Clone, Debug)]
pub struct FormatConfig {
    pub(crate) indent_unit: String,
    pub(crate) newline: String,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            indent_unit: "  ".to_string(),
            newline: "\n".to_string(),
        }
    }
}

pub trait Format {
    fn format(&self, node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode;
}

pub struct Formatter;

impl Formatter {
    pub fn format(node: GreenNode, config: FormatConfig) -> GreenNode {
        Self::format_with_depth(node, 0, config)
    }

    pub fn format_with_depth(node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode {
        match HjsonLanguage::kind_from_raw(node.kind()) {
            SyntaxKind::Object => ObjectFormatter.format(node, depth, config),
            SyntaxKind::Array => ArrayFormatter.format(node, depth, config),
            SyntaxKind::Pair => PairFormatter.format(node, depth, config),
            SyntaxKind::Value => ValueFormatter.format(node, depth, config),
            SyntaxKind::Root => RootFormatter.format(node, depth, config),
            _ => node,
        }
    }
}
