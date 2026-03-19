use rowan::{GreenNode, Language, NodeOrToken};

use crate::{
    format::{Format, FormatConfig, Formatter},
    syntax::ext::GreenElement,
    syntax::kind::{HjsonLanguage, SyntaxKind},
};

pub(crate) struct PairFormatter;

impl Format for PairFormatter {
    fn format(&self, node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode {
        let mut new_children: Vec<GreenElement> = Vec::with_capacity(node.children().count());

        let mut line_has_elements = false;
        let mut last_was_trivia = false;

        for child in node.children() {
            match child {
                NodeOrToken::Node(n) => {
                    if line_has_elements && !last_was_trivia {
                        new_children.push(config.create_space().into());
                    }
                    new_children.push(
                        Formatter::format_with_depth(n.to_owned(), depth, config.clone()).into(),
                    );
                    line_has_elements = true;
                    last_was_trivia = false;
                }

                NodeOrToken::Token(t) => match HjsonLanguage::kind_from_raw(t.kind()) {
                    SyntaxKind::Whitespace | SyntaxKind::Comma => {}

                    SyntaxKind::Newline => {
                        new_children.push(config.create_newline().into());
                        line_has_elements = false;
                        last_was_trivia = true;
                    }

                    SyntaxKind::Colon => {
                        new_children.push(t.to_owned().into());
                        line_has_elements = true;
                        last_was_trivia = false;
                    }

                    kind if kind.is_comment() => {
                        if line_has_elements {
                            if !last_was_trivia {
                                new_children.push(config.create_space().into());
                            }
                            new_children.push(t.to_owned().into());
                            line_has_elements = true;
                            last_was_trivia = false;
                        } else {
                            new_children.push(config.create_indent(depth + 1).into());
                            new_children.push(t.to_owned().into());
                            new_children.push(config.create_newline().into());
                            line_has_elements = false;
                            last_was_trivia = true;
                        }
                    }

                    _ => {
                        new_children.push(t.to_owned().into());
                        line_has_elements = true;
                        last_was_trivia = false;
                    }
                },
            }
        }

        SyntaxKind::Pair.create_node(new_children)
    }
}
