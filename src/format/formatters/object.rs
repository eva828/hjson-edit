use rowan::{GreenNode, Language, NodeOrToken};

use crate::{
    format::{Format, FormatConfig, Formatter},
    syntax::GreenElement,
    syntax::{HjsonLanguage, SyntaxKind},
};

pub(crate) struct ObjectFormatter;

impl Format for ObjectFormatter {
    fn format(&self, node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode {
        let mut new_children: Vec<GreenElement> = Vec::with_capacity(node.children().count());

        let mut line_has_elements = false;
        let mut has_content = false;

        for child in node.children() {
            match child {
                NodeOrToken::Node(n) => {
                    if line_has_elements {
                        new_children.push(config.create_newline().into());
                    }
                    new_children.push(config.create_indent(depth + 1).into());
                    new_children.push(
                        Formatter::format_with_depth(n.to_owned(), depth + 1, config.clone())
                            .into(),
                    );
                    line_has_elements = true;
                    has_content = true;
                }

                NodeOrToken::Token(t) => match HjsonLanguage::kind_from_raw(t.kind()) {
                    SyntaxKind::Whitespace | SyntaxKind::Comma => {}

                    SyntaxKind::Newline => {
                        if line_has_elements {
                            new_children.push(config.create_newline().into());
                            line_has_elements = false;
                        }
                    }

                    SyntaxKind::LBrace => {
                        new_children.push(t.to_owned().into());
                        new_children.push(config.create_newline().into());
                        line_has_elements = false;
                    }

                    SyntaxKind::RBrace => {
                        if has_content {
                            if line_has_elements {
                                new_children.push(config.create_newline().into());
                            }
                            new_children.push(config.create_indent(depth).into());
                        } else {
                            new_children.pop();
                        }
                        new_children.push(t.to_owned().into());
                        line_has_elements = true;
                    }

                    _ => {
                        if line_has_elements {
                            new_children.push(config.create_space().into());
                        } else {
                            new_children.push(config.create_indent(depth + 1).into());
                        }
                        new_children.push(t.to_owned().into());
                        line_has_elements = true;
                        has_content = true;
                    }
                },
            }
        }

        SyntaxKind::Object.create_node(new_children)
    }
}
