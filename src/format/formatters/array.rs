use rowan::{GreenNode, GreenToken, Language, NodeOrToken};

use crate::{
    format::{Format, FormatConfig, Formatter},
    syntax::green::GreenElement,
    syntax::kind::{HjsonLanguage, SyntaxKind},
};

pub(crate) struct ArrayFormatter;

impl Format for ArrayFormatter {
    fn format(&self, node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode {
        println!("[ArrayFormatter] Formatting value.");

        let mut new_children: Vec<GreenElement> = Vec::new();
        // その行にすでに何らかの要素（値やコメント）が出力されたかを管理するフラグ
        let mut line_has_elements = false;

        for child in node.children() {
            match &child {
                NodeOrToken::Node(child) => {
                    if line_has_elements {
                        new_children.push(NodeOrToken::Token(GreenToken::new(
                            SyntaxKind::Newline.into(),
                            &config.newline,
                        )));
                    }

                    new_children.push(NodeOrToken::Token(GreenToken::new(
                        SyntaxKind::Whitespace.into(),
                        &config.indent_unit.repeat(depth + 1),
                    )));
                    new_children.push(NodeOrToken::Node(Formatter::format_with_depth(
                        (*child).to_owned(),
                        depth + 1,
                        config.clone(),
                    )));
                    line_has_elements = true;
                }
                NodeOrToken::Token(child) => {
                    let kind = HjsonLanguage::kind_from_raw(child.kind());
                    match &kind {
                        SyntaxKind::Comma | SyntaxKind::Whitespace => {}

                        SyntaxKind::Newline => {
                            if line_has_elements {
                                new_children.push(NodeOrToken::Token(GreenToken::new(
                                    SyntaxKind::Newline.into(),
                                    &config.newline,
                                )));
                                line_has_elements = false;
                            }
                        }

                        SyntaxKind::LBracket => {
                            new_children.push(NodeOrToken::Token((*child).to_owned()));
                            new_children.push(NodeOrToken::Token(GreenToken::new(
                                SyntaxKind::Newline.into(),
                                &config.newline,
                            )));
                            line_has_elements = false;
                        }

                        SyntaxKind::RBracket => {
                            if line_has_elements {
                                new_children.push(NodeOrToken::Token(GreenToken::new(
                                    SyntaxKind::Newline.into(),
                                    &config.newline,
                                )));
                            }
                            new_children.push(NodeOrToken::Token(GreenToken::new(
                                SyntaxKind::Whitespace.into(),
                                &config.indent_unit.repeat(depth),
                            )));
                            new_children.push(NodeOrToken::Token((*child).to_owned()));
                            line_has_elements = true;
                        }

                        _ if kind.is_comment() => {
                            if line_has_elements {
                                new_children.push(NodeOrToken::Token(GreenToken::new(
                                    SyntaxKind::Whitespace.into(),
                                    " ",
                                )));
                            } else {
                                new_children.push(NodeOrToken::Token(GreenToken::new(
                                    SyntaxKind::Whitespace.into(),
                                    &config.indent_unit.repeat(depth + 1),
                                )));
                            }
                            new_children.push(NodeOrToken::Token((*child).to_owned()));
                            line_has_elements = true;
                        }

                        _ => {}
                    }
                }
            };
        }

        SyntaxKind::Array.create_node(new_children)
    }
}
