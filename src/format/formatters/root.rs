use rowan::{GreenNode, Language, NodeOrToken};

use crate::{
    format::{Format, FormatConfig, Formatter},
    syntax::GreenElement,
    syntax::{HjsonLanguage, SyntaxKind},
};

pub(crate) struct RootFormatter;

impl Format for RootFormatter {
    fn format(&self, node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode {
        let mut new_children: Vec<GreenElement> = Vec::with_capacity(node.children().count() + 1);

        let mut line_has_elements = false;

        for child in node.children() {
            match child {
                NodeOrToken::Node(n) => {
                    if line_has_elements {
                        new_children.push(config.create_newline().into());
                    }
                    new_children.push(
                        Formatter::format_with_depth(n.to_owned(), depth, config.clone()).into(),
                    );
                    line_has_elements = true;
                }

                NodeOrToken::Token(t) => match HjsonLanguage::kind_from_raw(t.kind()) {
                    SyntaxKind::Whitespace | SyntaxKind::Newline => {}

                    kind if kind.is_comment() => {
                        if line_has_elements {
                            new_children.push(config.create_newline().into());
                        }
                        new_children.push(t.to_owned().into());
                        new_children.push(config.create_newline().into());
                        line_has_elements = false;
                    }

                    _ => {
                        if line_has_elements {
                            new_children.push(config.create_newline().into());
                        }
                        new_children.push(t.to_owned().into());
                        line_has_elements = true;
                    }
                },
            }
        }

        if line_has_elements {
            new_children.push(config.create_newline().into());
        }

        SyntaxKind::Root.create_node(new_children)
    }
}
