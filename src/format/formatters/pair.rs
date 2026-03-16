// use crate::edit::high_level::{FormatError, RedNodeNavigateExt};
// use crate::format::dispatch::{Format, Formatter};
// use crate::format::util::child_indent;
// use crate::format::{FormatParams, FormatTargetUnit};
// use crate::syntax::green::GreenElement;
// use crate::syntax::kind::SyntaxKind;
// use crate::syntax::red::{RedNode, RedNodeCoreExt};
// use rowan::{GreenNode, NodeOrToken};

use rowan::GreenNode;

use crate::{format::Format, FormatConfig};

pub(crate) struct PairFormatter;

impl Format for PairFormatter {
    fn format(&self, node: GreenNode, depth: usize, config: FormatConfig) -> GreenNode {
        println!("[PairFormatter] Formatting value.");
        node
    }
}

// impl<'a> PairFormatter<'a> {
//     pub(crate) fn new(pair: &'a RedNode) -> Self {
//         Self { pair }
//     }
// }

// impl Format for PairFormatter<'_> {
//     fn format(&self, params: &FormatParams<'_>) -> Result<GreenNode, FormatError> {
//         let mut children = Vec::new();
//         let mut key_node = None;
//         let mut value_node = None;

//         for child in self.pair.children_with_tokens() {
//             if let NodeOrToken::Node(node) = child {
//                 match node.kind() {
//                     SyntaxKind::Key => key_node = Some(node),
//                     SyntaxKind::Value => value_node = Some(node),
//                     _ => {}
//                 }
//             }
//         }

//         let key = key_node.ok_or_else(|| FormatError::InvalidTarget {
//             target: FormatTargetUnit::Node(self.pair.path_from_root()),
//         })?;
//         let value = value_node.ok_or_else(|| FormatError::InvalidTarget {
//             target: FormatTargetUnit::Node(self.pair.path_from_root()),
//         })?;

//         children.extend(leading_comments_and_blank_lines(
//             self.pair,
//             params.indent,
//             params.newline,
//         ));
//         children.push(NodeOrToken::Node(key.to_green()));
//         children.push(NodeOrToken::Token(SyntaxKind::Colon.create_token(":")));

//         let formatted_value = Formatter::from_node(&value).format(params)?;
//         if pair_uses_multiline_value_layout(self.pair) {
//             let value_indent = child_indent(params.indent, params.indent_unit);
//             children.push(NodeOrToken::Token(
//                 SyntaxKind::Newline.create_token(params.newline),
//             ));
//             if !value_indent.is_empty() {
//                 children.push(NodeOrToken::Token(
//                     SyntaxKind::Whitespace.create_token(&value_indent),
//                 ));
//             }
//         } else {
//             children.push(NodeOrToken::Token(SyntaxKind::Whitespace.create_token(" ")));
//         }
//         children.push(NodeOrToken::Node(formatted_value));
//         children.extend(inline_comment_elements(self.pair));

//         Ok(SyntaxKind::Pair.create_node(children))
//     }
// }

// fn leading_comments_and_blank_lines(
//     pair: &RedNode,
//     indent: &str,
//     newline: &str,
// ) -> Vec<GreenElement> {
//     let mut result = Vec::new();
//     let mut pending_blank = 0usize;

//     for child in pair.children_with_tokens() {
//         let Some(token) = child.as_token() else {
//             break;
//         };
//         match token.kind() {
//             SyntaxKind::Newline => pending_blank += 1,
//             SyntaxKind::Whitespace => {}
//             SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment => {
//                 if pending_blank > 1 {
//                     result.push(NodeOrToken::Token(
//                         SyntaxKind::Newline.create_token(newline),
//                     ));
//                     if !indent.is_empty() {
//                         result.push(NodeOrToken::Token(
//                             SyntaxKind::Whitespace.create_token(indent),
//                         ));
//                     }
//                 }
//                 result.push(NodeOrToken::Token(token.kind().create_token(token.text())));
//                 result.push(NodeOrToken::Token(
//                     SyntaxKind::Newline.create_token(newline),
//                 ));
//                 if !indent.is_empty() {
//                     result.push(NodeOrToken::Token(
//                         SyntaxKind::Whitespace.create_token(indent),
//                     ));
//                 }
//                 pending_blank = 0;
//             }
//             _ => break,
//         }
//     }

//     result
// }

// fn pair_uses_multiline_value_layout(pair: &RedNode) -> bool {
//     let mut seen_colon = false;

//     for child in pair.children_with_tokens() {
//         match child {
//             NodeOrToken::Token(token) => match token.kind() {
//                 SyntaxKind::Colon => seen_colon = true,
//                 SyntaxKind::Whitespace if seen_colon => {}
//                 SyntaxKind::Newline if seen_colon => return true,
//                 _ => {}
//             },
//             NodeOrToken::Node(node) if seen_colon && node.kind() == SyntaxKind::Value => {
//                 return false;
//             }
//             _ => {}
//         }
//     }

//     false
// }

// fn inline_comment_elements(pair: &RedNode) -> Vec<GreenElement> {
//     let mut result = Vec::new();
//     let mut seen_value = false;
//     let mut last_was_space = false;

//     for child in pair.children_with_tokens() {
//         match child {
//             NodeOrToken::Node(node) if node.kind() == SyntaxKind::Value => {
//                 seen_value = true;
//             }
//             NodeOrToken::Token(token) if seen_value => match token.kind() {
//                 SyntaxKind::Whitespace => {
//                     last_was_space = true;
//                 }
//                 SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment => {
//                     if !last_was_space {
//                         result.push(NodeOrToken::Token(
//                             SyntaxKind::Whitespace.create_token("  "),
//                         ));
//                     }
//                     result.push(NodeOrToken::Token(token.kind().create_token(token.text())));
//                     last_was_space = false;
//                 }
//                 SyntaxKind::Newline => break,
//                 _ => {}
//             },
//             _ => {}
//         }
//     }

//     result
// }
